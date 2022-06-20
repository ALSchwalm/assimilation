use bevy::{core::FixedTimestep, prelude::*};
use bevy_prototype_lyon::prelude::*;
use rand::{thread_rng, Rng};
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::time::Duration;

mod core;

const PLAYER_COLOR: Color = Color::CYAN;
const BOT_COLOR: Color = Color::PINK;
const TILE_RADIUS: f32 = 15.0;
const TIME_STEP: f32 = 1.0 / 60.0;
const SCALE_FACTOR: f32 = 2.0;

fn point_inside_tile(tile_center: Vec2, point: Vec2) -> bool {
    let d = TILE_RADIUS * 2.0;
    let dx = (tile_center.x - point.x).abs() / d;
    let dy = (tile_center.y - point.y).abs() / d;
    let a = 0.25 * 3.0_f32.sqrt();
    (dy < a) && (a * dx + 0.25 * dy < 0.5 * a)
}

fn update_tile_colors(
    mut capture_events: EventReader<core::CaptureEvent>,
    players: Query<&core::Player>,
    mut tiles: Query<(&core::Tile, &mut DrawMode)>,
) {
    for capture in capture_events.iter() {
        for mut tile in tiles.iter_mut() {
            if capture.row == tile.0.row && capture.column == tile.0.column {
                let color = match players.get(capture.player) {
                    Ok(player) => match player.kind {
                        core::PlayerKind::Human => PLAYER_COLOR,
                        core::PlayerKind::Bot(_) => BOT_COLOR,
                    },
                    Err(_) => return,
                };
                *tile.1 = DrawMode::Outlined {
                    fill_mode: FillMode::color(color),
                    outline_mode: StrokeMode::new(Color::BLACK, 1.0),
                };
            }
        }
        println!("Capture of tile at {},{}", capture.row, capture.column);
    }
}

fn select_tile(
    mut selections: EventWriter<core::SelectEvent>,
    mouse_input: Res<Input<MouseButton>>,
    windows: Res<Windows>,
    players: Query<(Entity, &core::Player)>,
    tiles: Query<(&core::Tile, &Transform)>,
) {
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
    mut cursor_events: EventReader<CursorMoved>,
    mut tiles: Query<(&core::Tile, &mut Transform)>,
    windows: Res<Windows>,
) {
    let window = windows.primary();
    let offset_x = window.width() / 2.0;
    let offset_y = window.height() / 2.0;

    for event in cursor_events.iter() {
        let mouse_x = event.position.x - offset_x;
        let mouse_y = event.position.y - offset_y;

        for mut tile in tiles.iter_mut() {
            if point_inside_tile(
                Vec2::new(tile.1.translation.x, tile.1.translation.y),
                Vec2::new(mouse_x, mouse_y),
            ) {
                *tile.1 = tile.1.clone().with_scale(Vec3::new(1.1, 1.1, 0.0));
                tile.1.translation.z = 1.0;
            } else {
                *tile.1 = tile.1.clone().with_scale(Vec3::new(1.0, 1.0, 0.0));
                tile.1.translation.z = 0.0;
            }
        }
    }
}

fn setup(mut commands: Commands, windows: ResMut<Windows>) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(UiCameraBundle::default());

    change_resolution(windows);

    let player_id = commands
        .spawn()
        .insert(core::Player {
            score: 0,
            kind: core::PlayerKind::Human,
        })
        .id();
    let bot_id = commands
        .spawn()
        .insert(core::Player {
            score: 0,
            kind: core::PlayerKind::Bot(Timer::new(Duration::from_secs(1), false)),
        })
        .id();

    commands.insert_resource(core::GameState {
        players: vec![player_id, bot_id],
    });

    let shape = shapes::RegularPolygon {
        sides: 6,
        feature: shapes::RegularPolygonFeature::Radius(TILE_RADIUS),
        ..shapes::RegularPolygon::default()
    };

    let mut rng = thread_rng();

    let mut id_color_map = BTreeMap::new();
    id_color_map.insert(0, Color::RED);
    id_color_map.insert(1, Color::BLUE);
    id_color_map.insert(2, Color::YELLOW);
    id_color_map.insert(3, Color::GREEN);
    id_color_map.insert(4, Color::ORANGE);

    let board_rows = 10;
    let board_columns = 10;

    let board_x_offset = -(TILE_RADIUS * 3.0_f32.sqrt() * board_columns as f32) / 2.0;
    let board_y_offset = -(TILE_RADIUS * 1.5 * board_rows as f32) / 2.0;

    for row in 0..board_rows {
        for column in 0..board_columns {
            let column_offset = board_x_offset
                + if row % 2 == 0 {
                    TILE_RADIUS * 3.0_f32.sqrt() / 2.0
                } else {
                    0.0
                };
            let row_offset = board_y_offset;

            let initial_id = rng.gen_range(0..5);

            let (initial_color, initial_state) = if row == 0 && column == 0 {
                (PLAYER_COLOR, core::TileState::Owned(player_id))
            } else if row == 9 && column == 9 {
                (BOT_COLOR, core::TileState::Owned(bot_id))
            } else {
                (
                    id_color_map[&initial_id],
                    core::TileState::Unowned(initial_id),
                )
            };

            commands
                .spawn_bundle(GeometryBuilder::build_as(
                    &shape,
                    DrawMode::Outlined {
                        fill_mode: FillMode::color(initial_color),
                        outline_mode: StrokeMode::new(Color::BLACK, 1.0),
                    },
                    Transform::from_xyz(
                        column as f32 * TILE_RADIUS * 3.0_f32.sqrt() + column_offset,
                        row as f32 * TILE_RADIUS * 1.5 + row_offset,
                        0.0,
                    )
                    .with_rotation(Quat::from_rotation_z(PI as f32 / 6.0)),
                ))
                .insert(core::Tile {
                    row,
                    column,
                    state: initial_state,
                });
        }
    }
}

#[cfg(target_family = "wasm")]
fn change_resolution(mut windows: ResMut<Windows>) {
    let window = windows.primary_mut();
    window.update_scale_factor_from_backend(SCALE_FACTOR as f64);

    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();
    let width = body.client_width();
    let height = body.client_height();

    window.set_resolution(width as f32 / SCALE_FACTOR, height as f32 / SCALE_FACTOR);
}

#[cfg(not(target_family = "wasm"))]
fn change_resolution(mut windows: ResMut<Windows>) {
    let window = windows.primary_mut();
    window.update_scale_factor_from_backend(SCALE_FACTOR as f64);
}

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::rgb(0.4, 0.4, 0.4)))
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
        .run();
}
