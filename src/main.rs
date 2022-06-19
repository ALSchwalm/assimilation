use bevy::{core::FixedTimestep, prelude::*, sprite::MaterialMesh2dBundle};
use bevy_prototype_lyon::prelude::*;
use std::f64::consts::PI;
use std::collections::BTreeMap;
use rand::{Rng, thread_rng};

mod core;

const TILE_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
const TILE_RADIUS: f32 = 15.0;
const TIME_STEP: f32 = 1.0 / 60.0;

fn point_inside_tile(tile_center: Vec2, point: Vec2) -> bool {
    let d = TILE_RADIUS * 2.0;
    let dx = (tile_center.x - point.x).abs() / d;
    let dy = (tile_center.y - point.y).abs() / d;
    let a = 0.25 * 3.0_f32.sqrt();
    (dy < a) && (a * dx + 0.25 * dy < 0.5 * a)
}

fn update_tile_colors(
    mut capture_events: EventReader<core::CaptureEvent>,
    mut tiles: Query<(&core::Tile, &mut DrawMode)>,
) {
    for capture in capture_events.iter() {
        for mut tile in tiles.iter_mut() {
            if capture.row == tile.0.row && capture.column == tile.0.column {
                *tile.1 = DrawMode::Outlined {
                    fill_mode: FillMode::color(Color::CYAN),
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
                selections.send(core::SelectEvent {
                    row: tile.0.row,
                    column: tile.0.column,
                    player: player.0,
                })
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

fn setup(mut commands: Commands) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(UiCameraBundle::default());

    let player_id = commands.spawn().insert(core::Player { score: 0 }).id();

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
            let column_offset = board_x_offset + if row % 2 == 0 {
                TILE_RADIUS * 3.0_f32.sqrt() / 2.0
            } else {
                0.0
            };
            let row_offset = board_y_offset;

            let initial_id = rng.gen_range(0..5);

            let (initial_color, initial_state) = if row == 0 && column == 0 {
                (Color::CYAN, core::TileState::Owned(player_id))
            } else {
                (id_color_map[&initial_id], core::TileState::Unowned(initial_id))
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

    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();
    let width = body.client_width();
    let height = body.client_height();

    window.set_resolution(width as f32, height as f32);
}

#[cfg(not(target_family = "wasm"))]
fn change_resolution(_time: Res<Time>, _windows: ResMut<Windows>) {}

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .add_event::<CursorMoved>()
        .add_event::<core::SelectEvent>()
        .add_event::<core::CaptureEvent>()
        .add_plugins(DefaultPlugins)
        .add_plugin(ShapePlugin)
        .add_startup_system(setup)
        .add_system_set(SystemSet::new().with_run_criteria(FixedTimestep::step(TIME_STEP as f64)))
        .add_system(change_resolution)
        .add_system(hover_tile)
        .add_system(core::update_scores)
        .add_system(core::perform_selection.before(core::update_scores))
        .add_system(select_tile.before(core::perform_selection))
        .add_system(update_tile_colors.after(core::perform_selection))
        .run();
}
