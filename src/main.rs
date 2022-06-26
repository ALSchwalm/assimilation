use bevy::ecs::schedule::ShouldRun;
use bevy::{asset::AssetServerSettings, core::FixedTimestep, prelude::*};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use bevy_prototype_lyon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;

mod core;
mod levels;

const PLAYER_COLOR: Color = Color::CYAN;
const BOT_COLOR: Color = Color::PINK;
const TILE_RADIUS: f32 = 15.0;
const TIME_STEP: f32 = 1.0 / 60.0;
const SCALE_FACTOR: f32 = 2.0;

struct GameStartEvent {
    players: Vec<core::Player>,
    ids: BTreeMap<u32, Color>,
    level: &'static str,
    random: bool,
}

#[derive(Component)]
struct ScoreBoardEntry {
    player: Entity,
}

struct GameConfigState {
    level_name: &'static str,
    num_ids: u32,
}

#[derive(Component)]
struct WinnerText;

fn point_inside_tile(tile_center: Vec2, point: Vec2) -> bool {
    let d = TILE_RADIUS * 2.0;
    let dx = (tile_center.x - point.x).abs() / d;
    let dy = (tile_center.y - point.y).abs() / d;
    let a = 0.25 * 3.0_f32.sqrt();
    (dy < a) && (a * dx + 0.25 * dy < 0.5 * a)
}

fn update_scoreboard(
    state: Res<core::GameState>,
    players: Query<&core::Player>,
    mut scores: Query<(&ScoreBoardEntry, &mut Text), Without<WinnerText>>,
    mut winner_display: Query<(&mut WinnerText, &mut Text)>,
) {
    for mut score in scores.iter_mut() {
        let player = match players.get(score.0.player) {
            Ok(player) => player,
            Err(_) => continue,
        };

        score.1.sections[0].value = format!("{} Score: {}", player.name, player.score);
    }

    let winner_id = match state.phase {
        core::GamePhase::Over(id) => id,
        _ => return,
    };

    let winner = match players.get(winner_id) {
        Ok(player) => player,
        Err(_) => return,
    };

    let mut display = winner_display
        .iter_mut()
        .next()
        .expect("Missing winner display");
    display.1.sections[0].value = format!("Winner: {}", winner.name);
}

fn update_tile_colors(
    mut capture_events: EventReader<core::CaptureEvent>,
    players: Query<&core::Player>,
    mut tiles: Query<(&core::Tile, &mut DrawMode, &mut Transform)>,
) {
    //TODO: just redo all tile colors if there has been a capture
    for capture in capture_events.iter() {
        for mut tile in tiles.iter_mut() {
            if capture.row == tile.0.row && capture.column == tile.0.column {
                let color = match players.get(capture.player) {
                    Ok(player) => player.color,
                    Err(_) => return,
                };
                *tile.1 = DrawMode::Outlined {
                    fill_mode: FillMode::color(color),
                    outline_mode: StrokeMode::new(Color::WHITE, 1.0),
                };
                tile.2.translation.z = 1.0;
            }
        }
    }
}

fn select_tile(
    state: Res<core::GameState>,
    mut selections: EventWriter<core::SelectEvent>,
    mouse_input: Res<Input<MouseButton>>,
    windows: Res<Windows>,
    players: Query<(Entity, &core::Player)>,
    mut tiles: Query<(&mut core::Tile, &Transform)>,
) {
    match state.phase {
        core::GamePhase::Over(_) => return,
        _ => (),
    }

    let window = windows.primary();
    let player = players.get(state.players[0]).expect("Missing player");

    if mouse_input.just_pressed(MouseButton::Left) {
        let pos = if let Some(pos) = window.cursor_position() {
            pos
        } else {
            return;
        };
        let offset_x = window.width() / 2.0;
        let offset_y = window.height() / 2.0;

        let mouse_x = pos.x - offset_x;
        let mouse_y = pos.y - offset_y;

        let tile = tiles
            .iter()
            .find(|tile| {
                point_inside_tile(
                    Vec2::new(tile.1.translation.x, tile.1.translation.y),
                    Vec2::new(mouse_x, mouse_y),
                )
            })
            .map(|tile| tile.0.clone());

        let tile = if let Some(tile) = tile {
            tile
        } else {
            return;
        };

        match tile.state {
            core::TileState::Unowned(id) => {
                let mut valid = false;
                core::for_each_selected_tile(
                    tiles.iter_mut().map(|t| t.0).collect(),
                    id,
                    state.players[0],
                    |valid_tile| {
                        if valid_tile.row == tile.row && valid_tile.column == tile.column {
                            valid = true;
                        }
                    },
                );
                if valid {
                    selections.send(core::SelectEvent {
                        id,
                        player: player.0,
                    })
                }
            }
            _ => (),
        }
    }
}

fn hover_tile(
    state: Res<core::GameState>,
    players: Query<&core::Player>,
    mut cursor_events: EventReader<CursorMoved>,
    mut tiles: Query<(&mut core::Tile, &mut DrawMode, &mut Transform)>,
    windows: Res<Windows>,
) {
    let window = windows.primary();
    let offset_x = window.width() / 2.0;
    let offset_y = window.height() / 2.0;

    match state.phase {
        core::GamePhase::Over(_) => return,
        _ => (),
    }

    let (player_id, player_color) = match players.get(state.players[0]) {
        Ok(player) => match player.kind {
            core::PlayerKind::Human => (state.players[0], player.color),
            _ => return,
        },
        Err(_) => return,
    };

    let mut done_reset = false;
    for event in cursor_events.iter() {
        let mouse_x = event.position.x - offset_x;
        let mouse_y = event.position.y - offset_y;

        // Only reset the positions once, and only do it if there has been some
        // mouse movement
        if !done_reset {
            for mut tile in tiles.iter_mut() {
                let (color, border, zpos) = match tile.0.state {
                    core::TileState::Unowned(id) => (state.ids[&id], Color::BLACK, 0.0),
                    core::TileState::Owned(player) => {
                        let player = players.get(player).expect("Missing player");
                        (player.color, Color::WHITE, 1.0)
                    }
                    _ => continue,
                };

                *tile.1 = DrawMode::Outlined {
                    fill_mode: FillMode::color(color),
                    outline_mode: StrokeMode::new(border, 1.0),
                };
                tile.2.translation.z = zpos;
            }
            done_reset = true;
        }

        let mut hover_info = None;
        for tile in tiles.iter() {
            if point_inside_tile(
                Vec2::new(tile.2.translation.x, tile.2.translation.y),
                Vec2::new(mouse_x, mouse_y),
            ) {
                match tile.0.state {
                    core::TileState::Unowned(id) => {
                        hover_info = Some((id, tile.0.row, tile.0.column))
                    }
                    _ => continue,
                }
            }
        }

        let hover_info = if let Some(hover_info) = hover_info {
            hover_info
        } else {
            return;
        };

        let mut is_reachable = false;
        let mut selected_tiles = BTreeSet::new();
        core::for_each_selected_tile(
            tiles.iter_mut().map(|t| t.0).collect(),
            hover_info.0,
            player_id,
            |tile| {
                if tile.row == hover_info.1 && tile.column == hover_info.2 {
                    is_reachable = true
                }
                selected_tiles.insert((tile.row, tile.column));
            },
        );

        if !is_reachable {
            return;
        }

        for mut tile in tiles.iter_mut() {
            if selected_tiles.contains(&(tile.0.row, tile.0.column)) {
                let (color, border) = match tile.0.state {
                    core::TileState::Owned(_) => (player_color, Color::WHITE),
                    core::TileState::Unowned(_) => {
                        let mut color = player_color.as_hsla();
                        match color {
                            Color::Hsla {
                                hue: _,
                                ref mut saturation,
                                ref mut lightness,
                                alpha: _,
                            } => {
                                *lightness = 0.6;
                                *saturation = 0.6;
                            }
                            _ => unreachable!(),
                        }
                        (color, Color::rgb(0.9, 0.9, 0.9))
                    }
                    _ => panic!("Invalid hovered tile"),
                };
                *tile.1 = DrawMode::Outlined {
                    fill_mode: FillMode::color(color),
                    outline_mode: StrokeMode::new(border, 1.0),
                };
                tile.2.translation.z = 1.0;
            }
        }
    }
}

fn game_start(
    mut gamestate: ResMut<core::GameState>,
    mut start_event: EventReader<GameStartEvent>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for start_settings in start_event.iter() {
        let ids = start_settings
            .players
            .clone()
            .into_iter()
            .map(|player| commands.spawn().insert(player).id());

        gamestate.phase = core::GamePhase::Running;
        gamestate.ids = start_settings.ids.clone();
        gamestate.players = ids.collect();

        let tiles = core::load_level(
            start_settings.level,
            &gamestate.players,
            gamestate.ids.keys().cloned().collect(),
            true,
        );

        let shape = shapes::RegularPolygon {
            sides: 6,
            feature: shapes::RegularPolygonFeature::Radius(TILE_RADIUS),
            ..shapes::RegularPolygon::default()
        };

        let (max_row, max_column) = tiles
            .iter()
            .map(|tile| (tile.row, tile.column))
            .max()
            .expect("Unable to get board dimensions");

        let board_rows = max_row + 1;
        let board_columns = max_column + 1;
        let board_x_offset = -(TILE_RADIUS * 3.0_f32.sqrt() * board_columns as f32) / 2.0;
        let board_y_offset = (TILE_RADIUS * 1.5 * board_rows as f32) / 2.0;

        for tile in tiles {
            let row = tile.row;
            let column = tile.column;

            let column_offset = board_x_offset
                + if row % 2 == 0 {
                    TILE_RADIUS * 3.0_f32.sqrt() / 2.0
                } else {
                    0.0
                };
            let row_offset = board_y_offset;

            let (initial_color, border_color, z_pos) = match tile.state {
                core::TileState::Owned(id) => {
                    let idx = gamestate
                        .players
                        .iter()
                        .position(|player_id| *player_id == id)
                        .expect("Unknown player id");

                    (start_settings.players[idx].color, Color::WHITE, 1.0)
                }
                core::TileState::Unowned(id) => (gamestate.ids[&id], Color::BLACK, 0.0),
                core::TileState::Empty => {
                    commands.spawn().insert(tile);
                    continue;
                }
            };

            commands
                .spawn_bundle(GeometryBuilder::build_as(
                    &shape,
                    DrawMode::Outlined {
                        fill_mode: FillMode::color(initial_color),
                        outline_mode: StrokeMode::new(border_color, 1.0),
                    },
                    Transform::from_xyz(
                        column as f32 * TILE_RADIUS * 3.0_f32.sqrt() + column_offset,
                        row_offset - row as f32 * TILE_RADIUS * 1.5,
                        z_pos,
                    )
                    .with_rotation(Quat::from_rotation_z(PI as f32 / 6.0)),
                ))
                .insert(tile);
        }

        commands
            .spawn_bundle(NodeBundle {
                style: Style {
                    size: Size::new(Val::Percent(100.0), Val::Px(20.0)),
                    justify_content: JustifyContent::Center,
                    margin: Rect {
                        bottom: Val::Px(50.0),
                        ..default()
                    },
                    ..default()
                },
                color: Color::NONE.into(),
                ..default()
            })
            .with_children(|parent| {
                parent
                    .spawn_bundle(NodeBundle {
                        style: Style {
                            size: Size::new(Val::Px(300.0), Val::Px(50.0)),
                            border: Rect::all(Val::Px(2.0)),
                            align_content: AlignContent::Center,
                            ..default()
                        },
                        color: Color::NONE.into(),
                        ..default()
                    })
                    .with_children(|parent| {
                        parent
                            .spawn_bundle(TextBundle {
                                style: Style {
                                    margin: Rect::all(Val::Px(5.0)),
                                    ..default()
                                },
                                text: Text::with_section(
                                    "",
                                    TextStyle {
                                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                        font_size: 10.0,
                                        color: Color::WHITE,
                                    },
                                    Default::default(),
                                ),
                                ..default()
                            })
                            .insert(ScoreBoardEntry {
                                player: gamestate.players[0],
                            });

                        parent
                            .spawn_bundle(TextBundle {
                                style: Style {
                                    margin: Rect::all(Val::Px(5.0)),
                                    ..default()
                                },
                                text: Text::with_section(
                                    "",
                                    TextStyle {
                                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                        font_size: 10.0,
                                        color: Color::WHITE,
                                    },
                                    Default::default(),
                                ),
                                ..default()
                            })
                            .insert(WinnerText);

                        parent
                            .spawn_bundle(TextBundle {
                                style: Style {
                                    margin: Rect::all(Val::Px(5.0)),
                                    ..default()
                                },
                                text: Text::with_section(
                                    "",
                                    TextStyle {
                                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                        font_size: 10.0,
                                        color: Color::WHITE,
                                    },
                                    Default::default(),
                                ),
                                ..default()
                            })
                            .insert(ScoreBoardEntry {
                                player: gamestate.players[1],
                            });
                    });
            });
    }
}

fn setup(mut commands: Commands, mut windows: ResMut<Windows>) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(UiCameraBundle::default());

    set_scale(&mut windows);

    commands.insert_resource(GameConfigState {
        level_name: "Hexagon",
        num_ids: 5,
    });

    commands.insert_resource(core::GameState {
        players: vec![],
        phase: core::GamePhase::Config,
        ids: BTreeMap::new(),
    });
}

#[cfg(target_family = "wasm")]
fn get_asset_location() -> AssetServerSettings {
    AssetServerSettings::default()
}

#[cfg(not(target_family = "wasm"))]
fn get_asset_location() -> AssetServerSettings {
    AssetServerSettings {
        asset_folder: "./site/assets".into(),
        ..default()
    }
}

#[cfg(target_family = "wasm")]
fn set_scale(windows: &mut ResMut<Windows>) {
    let window = windows.primary_mut();
    window.update_scale_factor_from_backend(SCALE_FACTOR as f64);

    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();
    let width = body.client_width();
    let height = body.client_height();

    window.set_resolution(width as f32 / SCALE_FACTOR, height as f32 / SCALE_FACTOR);
}

#[cfg(not(target_family = "wasm"))]
fn set_scale(windows: &mut ResMut<Windows>) {
    let window = windows.primary_mut();
    window.update_scale_factor_from_backend(SCALE_FACTOR as f64);
}

fn run_if_game_started(state: Res<core::GameState>) -> ShouldRun {
    match state.phase {
        core::GamePhase::Running => ShouldRun::Yes,
        _ => ShouldRun::No,
    }
}

fn show_title(
    mut config: ResMut<GameConfigState>,
    state: Res<core::GameState>,
    mut egui_ctx: ResMut<EguiContext>,
    mut game_start: EventWriter<GameStartEvent>,
) {
    match state.phase {
        core::GamePhase::Config => (),
        _ => return,
    }

    egui::Area::new("main")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(egui_ctx.ctx_mut(), |ui| {
            ui.label(egui::RichText::new("Assimilation").size(30.0));
            ui.add_space(30.0);

            egui::ComboBox::from_label("Level")
                .selected_text(format!("{}", config.as_mut().level_name))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut config.as_mut().level_name, "Hexagon", "Hexagon");
                    ui.selectable_value(&mut config.as_mut().level_name, "Square", "Square");
                });

            ui.add(egui::Slider::new(&mut config.as_mut().num_ids, 2..=6).text("Colors"));

            if ui.button("start").clicked() {
                let player = core::Player {
                    name: "Player".into(),
                    score: 0,
                    kind: core::PlayerKind::Human,
                    color: PLAYER_COLOR,
                };
                let bot = core::Player {
                    name: "Bot".into(),
                    score: 0,
                    kind: core::PlayerKind::Bot,
                    color: BOT_COLOR,
                };

                let ids = BTreeMap::from([
                    (0, Color::hex("483DDB").unwrap()),
                    (1, Color::hex("DB3E3A").unwrap()),
                    (2, Color::hex("68DB48").unwrap()),
                    (3, Color::hex("DBC132").unwrap()),
                    (4, Color::hex("DB8259").unwrap()),
                    (5, Color::hex("A121B8").unwrap()),
                ]);

                let selected_ids = ids
                    .into_iter()
                    .filter(|(k, _)| (0..config.num_ids).contains(k))
                    .collect();

                let level = match config.level_name {
                    "Square" => levels::SQUARE,
                    "Hexagon" => levels::HEXAGON,
                    _ => panic!("Unknown level"),
                };

                game_start.send(GameStartEvent {
                    players: vec![player, bot],
                    level,
                    ids: selected_ids,
                    random: false,
                });
            }
        });
}

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::rgb(0.4, 0.4, 0.4)))
        .insert_resource(get_asset_location())
        .add_event::<CursorMoved>()
        .add_event::<core::SelectEvent>()
        .add_event::<core::CaptureEvent>()
        .add_event::<GameStartEvent>()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_plugin(ShapePlugin)
        .add_startup_system(setup)
        .add_system_set(SystemSet::new().with_run_criteria(FixedTimestep::step(TIME_STEP as f64)))
        .add_system(show_title)
        .add_system(game_start)
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(run_if_game_started)
                .with_system(hover_tile)
                .with_system(core::update_scores)
                .with_system(core::perform_selection.before(core::update_scores))
                .with_system(core::perform_ai_move.before(select_tile))
                .with_system(select_tile.before(core::perform_selection))
                .with_system(update_tile_colors.after(core::perform_selection))
                .with_system(update_scoreboard.after(core::update_scores)),
        )
        .run();
}
