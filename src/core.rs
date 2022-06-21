use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Debug)]
pub enum TileState {
    Empty,
    Owned(Entity),
    Unowned(u32),
}

#[derive(Component)]
pub struct Tile {
    pub row: i32,
    pub column: i32,
    pub state: TileState,
}

pub struct SelectEvent {
    pub id: u32,
    pub player: Entity,
}

pub struct CaptureEvent {
    pub row: i32,
    pub column: i32,
    pub player: Entity,
}

pub enum PlayerKind {
    Human,
    Bot(Timer),
}

#[derive(Component)]
pub struct Player {
    pub name: String,
    pub kind: PlayerKind,
    pub score: u32,
}

pub enum GamePhase {
    Running,
    Over(Entity),
}

pub struct GameState {
    // The head of this vec is always the 'current' player
    pub players: Vec<Entity>,
    pub phase: GamePhase,
}

pub fn for_each_selected_tile<T>(
    mut tiles: Vec<T>,
    selection: u32,
    player: Entity,
    mut callback: impl FnMut(&mut Tile),
) where
    T: core::ops::DerefMut<Target = Tile>,
{
    let mut owned_tiles = tiles
        .iter()
        .filter(|tile| match tile.state {
            TileState::Owned(owner) => owner == player,
            _ => false,
        })
        .map(|tile| (tile.row, tile.column))
        .collect::<HashSet<(i32, i32)>>();

    loop {
        let mut did_capture = false;

        for mut tile in tiles.iter_mut() {
            println!("Considering tile at {},{}", tile.row, tile.column);

            if owned_tiles.contains(&(tile.row, tile.column)) {
                continue;
            }

            // TODO: if we capture (or skip) we should jump out to the next tile
            for row_offset in [-1, 0, 1] {
                for column_offset in [-1, 0, 1] {
                    if row_offset == 0 && column_offset == 0 {
                        continue;
                    }

                    if tile.row % 2 == 0 && row_offset != 0 && column_offset == -1 {
                        continue;
                    }

                    if tile.row % 2 != 0 && row_offset != 0 && column_offset == 1 {
                        continue;
                    }

                    if !owned_tiles.contains(&(tile.row + row_offset, tile.column + column_offset))
                    {
                        continue;
                    }

                    println!(
                        "  candidate because owned tile at {},{}",
                        tile.row + row_offset,
                        tile.column + column_offset
                    );

                    match tile.state {
                        TileState::Unowned(id) => {
                            if id == selection {
                                owned_tiles.insert((tile.row, tile.column));
                                did_capture = true;
                                callback(&mut tile);
                                println!("  captured tile at {},{}", tile.row, tile.column);
                            } else {
                                println!(
                                    "  no capture because id = {}, but selection = {}",
                                    id, selection
                                );
                            }
                        }
                        ref state => {
                            println!("  no capture because tile state = {:?}", state);
                            continue;
                        }
                    }
                }
            }
        }

        if !did_capture {
            break;
        }
    }
}

pub fn perform_ai_move(
    state: Res<GameState>,
    players: Query<&Player>,
    mut selections: EventWriter<SelectEvent>,
    mut tiles: Query<&mut Tile>,
) {
    let player = match players.get(state.players[0]) {
        Ok(player) => match player.kind {
            PlayerKind::Bot(_) => state.players[0],
            _ => return,
        },
        Err(_) => return,
    };

    println!("First player is a bot. Making a move");

    let mut best_score = 0;
    let mut best_move = 0;
    for id in 0..5 {
        let mut score = 0;
        for_each_selected_tile(tiles.iter_mut().collect(), id, player, |_| {
            score += 1;
        });
        if score > best_score {
            best_score = score;
            best_move = id;
        }
    }

    selections.send(SelectEvent {
        player,
        id: best_move,
    });
}

pub fn update_scores(mut players: Query<&mut Player>, tiles: Query<&Tile>) {
    for mut player in players.iter_mut() {
        player.score = 0;
    }

    for tile in tiles.iter() {
        match tile.state {
            TileState::Owned(player) => {
                if let Ok(mut player) = players.get_mut(player) {
                    player.score += 1;
                }
            }
            _ => continue,
        }
    }
}

pub fn perform_selection(
    mut state: ResMut<GameState>,
    mut selections: EventReader<SelectEvent>,
    mut tiles: Query<&mut Tile>,
    mut captures: EventWriter<CaptureEvent>,
) {
    for selection in selections.iter() {
        for_each_selected_tile(
            tiles.iter_mut().collect(),
            selection.id,
            selection.player,
            |tile| {
                tile.state = TileState::Owned(selection.player);
                println!("  capture of tile at {},{}", tile.row, tile.column);
                captures.send(CaptureEvent {
                    row: tile.row,
                    column: tile.column,
                    player: selection.player,
                });
            },
        );

        state.players.rotate_right(1);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bevy::ecs::event::Events;

    #[test]
    fn do_selection() {
        // Setup app
        let mut app = App::new();

        let player_id = app
            .world
            .spawn()
            .insert(Player {
                name: "Player".into(),
                score: 0,
                kind: PlayerKind::Human,
            })
            .id();

        for row in 0..10 {
            for column in 0..10 {
                app.world.spawn().insert(Tile {
                    row,
                    column,
                    state: TileState::Unowned(0),
                });
            }
        }

        app.add_event::<CaptureEvent>();
        app.add_event::<SelectEvent>();
        app.insert_resource(GameState {
            players: vec![player_id],
            phase: GamePhase::Running,
        });
        app.add_system(update_scores);
        app.add_system(perform_selection.before(update_scores));

        let mut events = app.world.resource_mut::<Events<SelectEvent>>();
        events.send(SelectEvent {
            player: player_id,
            id: 0,
        });

        // Run systems
        app.update();

        let player = app
            .world
            .query::<&Player>()
            .iter(&app.world)
            .next()
            .unwrap();

        // The play owns no tiles, so should have no score
        assert_eq!(player.score, 0);
    }
}
