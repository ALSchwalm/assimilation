use bevy::{asset::AssetServerSettings, core::FixedTimestep, prelude::*};
use bevy_prototype_lyon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use std::time::Duration;

mod core;
mod levels;

const PLAYER_COLOR: Color = Color::CYAN;
const BOT_COLOR: Color = Color::PINK;
const TILE_RADIUS: f32 = 15.0;
const TIME_STEP: f32 = 1.0 / 60.0;
const SCALE_FACTOR: f32 = 2.0;

#[derive(Component)]
struct ScoreBoardEntry {
    player: Entity,
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
        println!("Capture of tile at {},{}", capture.row, capture.column);
    }
}

fn select_tile(
    state: Res<core::GameState>,
    mut selections: EventWriter<core::SelectEvent>,
    mouse_input: Res<Input<MouseButton>>,
    windows: Res<Windows>,
    players: Query<(Entity, &core::Player)>,
    tiles: Query<(&core::Tile, &Transform)>,
) {
    match state.phase {
        core::GamePhase::Over(_) => return,
        _ => (),
    }

    let window = windows.primary();
    let player = players.iter().next().expect("Missing player");

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

        for tile in tiles.iter() {
            if point_inside_tile(
                Vec2::new(tile.1.translation.x, tile.1.translation.y),
                Vec2::new(mouse_x, mouse_y),
            ) {
                match tile.0.state {
                    core::TileState::Unowned(id) => selections.send(core::SelectEvent {
                        id,
                        player: player.0,
                    }),
                    _ => (),
                }
            }
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

fn setup(mut commands: Commands, mut windows: ResMut<Windows>, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(UiCameraBundle::default());

    set_scale(&mut windows);

    let player_id = commands
        .spawn()
        .insert(core::Player {
            name: "Player".into(),
            score: 0,
            kind: core::PlayerKind::Human,
            color: PLAYER_COLOR,
        })
        .id();
    let bot_id = commands
        .spawn()
        .insert(core::Player {
            name: "Bot".into(),
            score: 0,
            kind: core::PlayerKind::Bot(Timer::new(Duration::from_secs(1), false)),
            color: BOT_COLOR,
        })
        .id();

    // TODO: this should just be in the gamestate
    let id_color_map = BTreeMap::from([
        (0, Color::hex("483DDB").unwrap()),
        (1, Color::hex("DB3E3A").unwrap()),
        (2, Color::hex("68DB48").unwrap()),
        (3, Color::hex("DBC132").unwrap()),
        (4, Color::hex("DB8259").unwrap()),
    ]);

    let gamestate = core::GameState {
        players: vec![player_id, bot_id],
        phase: core::GamePhase::Running,
        ids: id_color_map.clone(),
    };

    let tiles = core::load_level(
        levels::RING,
        &gamestate.players,
        gamestate.ids.keys().cloned().collect(),
        true,
    );

    commands.insert_resource(gamestate);

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
                if id == player_id {
                    (PLAYER_COLOR, Color::WHITE, 1.0)
                } else {
                    (BOT_COLOR, Color::WHITE, 1.0)
                }
            }
            core::TileState::Unowned(id) => (id_color_map[&id], Color::BLACK, 0.0),
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
                        .insert(ScoreBoardEntry { player: player_id });

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
                        .insert(ScoreBoardEntry { player: bot_id });
                });
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

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::rgb(0.4, 0.4, 0.4)))
        .insert_resource(get_asset_location())
        .add_event::<CursorMoved>()
        .add_event::<core::SelectEvent>()
        .add_event::<core::CaptureEvent>()
        .add_plugins(DefaultPlugins)
        .add_plugin(ShapePlugin)
        .add_startup_system(setup)
        .add_system_set(SystemSet::new().with_run_criteria(FixedTimestep::step(TIME_STEP as f64)))
        .add_system(hover_tile)
        .add_system(core::update_scores)
        .add_system(core::perform_selection.before(core::update_scores))
        .add_system(core::perform_ai_move.before(select_tile))
        .add_system(select_tile.before(core::perform_selection))
        .add_system(update_tile_colors.after(core::perform_selection))
        .add_system(update_scoreboard.after(core::update_scores))
        .run();
}
