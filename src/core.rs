use bevy::prelude::*;
use rand::{seq::SliceRandom, thread_rng};
use std::collections::{BTreeMap, HashSet};

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

#[derive(Clone)]
pub enum GamePhase {
    Running,
    Over(Entity),
}

#[derive(Clone)]
pub struct GameState {
    // The head of this vec is always the 'current' player
    pub players: Vec<Entity>,
    pub phase: GamePhase,
    pub ids: BTreeMap<u32, Color>,
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

                    match tile.state {
                        TileState::Unowned(id) => {
                            if id == selection {
                                owned_tiles.insert((tile.row, tile.column));
                                did_capture = true;
                                callback(&mut tile);
                            }
                        }
                        _ => {
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
    for id in 0..state.ids.len() as u32 {
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

pub fn update_scores(
    mut state: ResMut<GameState>,
    mut players: Query<(Entity, &mut Player)>,
    mut tiles: Query<&mut Tile>,
) {
    for mut player in players.iter_mut() {
        player.1.score = 0;
    }

    let mut total_unowned = 0;
    for tile in tiles.iter() {
        match tile.state {
            TileState::Owned(player) => {
                if let Ok(mut player) = players.get_mut(player) {
                    player.1.score += 1;
                }
            }
            TileState::Unowned(_) => total_unowned += 1,
            _ => continue,
        }
    }

    //For now, the game is over if either player can't move
    let mut player_no_moves = None;
    for player in players.iter() {
        let mut possible_captures = false;
        for possible_selection in state.ids.keys() {
            for_each_selected_tile(
                tiles.iter_mut().collect(),
                *possible_selection,
                player.0,
                |_| {
                    possible_captures = true;
                },
            );
        }

        if !possible_captures {
            player_no_moves = Some(player.0);
            break;
        }
    }

    let player_no_moves = if let Some(player_no_moves) = player_no_moves {
        player_no_moves
    } else {
        return;
    };

    for mut player in players.iter_mut() {
        if player.0 != player_no_moves {
            player.1.score += total_unowned;
            break;
        }
    }

    let winner = players
        .iter()
        .max_by(|player1, player2| player1.1.score.cmp(&player2.1.score))
        .expect("Missing winner");

    state.phase = GamePhase::Over(winner.0);
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

pub fn load_level(
    level: impl AsRef<str>,
    players: &[Entity],
    ids: Vec<u32>,
    _random: bool,
) -> Vec<Tile> {
    //TODO: check the level is square
    let mut tiles = vec![];
    let level = level.as_ref().trim();
    for (row, line) in level.lines().enumerate() {
        for (column, tile_desc) in line.split_whitespace().enumerate() {
            let row = row as i32;
            let column = column as i32;
            let state = match tile_desc {
                "-" => TileState::Empty,
                "|" => TileState::Unowned(
                    *ids.as_slice()
                        .choose(&mut thread_rng())
                        .expect("Unable to make choice"),
                ),
                val => {
                    let player_num: usize = val
                        .parse()
                        .expect(&format!("Unexpected value in level: {}", val));

                    if player_num == 0 || player_num - 1 >= players.len() {
                        panic!(
                            "Invalid player number in level: {} (max {})",
                            val,
                            players.len()
                        );
                    }
                    TileState::Owned(players[player_num - 1])
                }
            };
            tiles.push(Tile { row, column, state })
        }
    }
    tiles
}

#[cfg(test)]
mod test {
    use super::*;
    use bevy::ecs::event::Events;
    use std::time::Duration;

    fn test_app_setup() -> (App, GameState) {
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

        let bot_id = app
            .world
            .spawn()
            .insert(Player {
                name: "Bot".into(),
                score: 0,
                kind: PlayerKind::Bot(Timer::new(Duration::from_secs(1), false)),
            })
            .id();

        let state = GameState {
            players: vec![player_id, bot_id],
            phase: GamePhase::Running,
            ids: BTreeMap::from([(0, Color::GREEN), (1, Color::YELLOW)]),
        };

        app.add_event::<CaptureEvent>();
        app.add_event::<SelectEvent>();
        app.insert_resource(state.clone());
        app.add_system(update_scores);
        app.add_system(perform_selection.before(update_scores));

        (app, state)
    }

    #[test]
    fn test_load_level_basic() {
        let (_, state) = test_app_setup();

        let desc = r#"
- 1 | | | 2 -
- | | | | | -
"#;
        let tiles = load_level(
            desc,
            &state.players,
            state.ids.keys().cloned().collect(),
            true,
        );

        assert_eq!(tiles.len(), 14);
    }

    #[test]
    fn do_selection() {
        let (mut app, state) = test_app_setup();

        for row in 0..10 {
            for column in 0..10 {
                app.world.spawn().insert(Tile {
                    row,
                    column,
                    state: TileState::Unowned(0),
                });
            }
        }

        let mut events = app.world.resource_mut::<Events<SelectEvent>>();
        events.send(SelectEvent {
            player: state.players[0],
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
