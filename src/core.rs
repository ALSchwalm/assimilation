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
    pub row: i32,
    pub column: i32,
    pub player: Entity,
}

pub struct CaptureEvent {
    pub row: i32,
    pub column: i32,
    pub player: Entity,
}

#[derive(Component)]
pub struct Player {
    pub score: u32,
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

pub fn perform_selection(mut selections: EventReader<SelectEvent>,
                     mut tiles: Query<&mut Tile>,
                     mut captures: EventWriter<CaptureEvent>) {
    for selection in selections.iter() {
        let selected_tile = tiles
            .iter()
            .find(|tile| tile.row == selection.row && tile.column == selection.column);
        let selected_id = if let Some(tile) = selected_tile {
            match tile.state {
                TileState::Unowned(id) => id,
                _ => continue,
            }
        } else {
            //TODO: log this
            continue;
        };

        loop {
            let mut did_capture = false;
            let owned_tiles = tiles
                .iter()
                .filter(|tile| match tile.state {
                    TileState::Owned(owner) => owner == selection.player,
                    _ => false
                })
                .map(|tile| (tile.row, tile.column))
                .collect::<HashSet<(i32, i32)>>();

            for mut tile in tiles.iter_mut() {

                println!("Considering tile at {},{}", tile.row, tile.column);

                for row_offset in [-1, 0, 1] {
                    for column_offset in [-1, 0, 1] {
                        if tile.row % 2 == 0 && row_offset != 0 && column_offset == -1 {
                            continue;
                        }

                        if tile.row % 2 != 0 && row_offset != 0 && column_offset == 1 {
                            continue;
                        }

                        if !owned_tiles
                            .contains(&(tile.row + row_offset, tile.column + column_offset))
                        {
                            continue;
                        }

                        println!("  candidate because owned tile at {},{}", tile.row + row_offset, tile.column + column_offset);

                        match tile.state {
                            TileState::Unowned(id) => {
                                if id == selected_id {
                                    tile.state = TileState::Owned(selection.player);
                                    println!("  capture of tile at {},{}", tile.row, tile.column);
                                    captures.send(CaptureEvent {
                                        row: tile.row,
                                        column: tile.column,
                                        player: selection.player
                                    });
                                    did_capture = true;
                                } else {
                                    println!("  no capture because id = {}, but selection = {}", id, selected_id);
                                }
                            }
                            ref state => {
                                println!("  no capture because tile state = {:?}", state);
                                continue
                            },
                        }
                    }
                }
            }

            if !did_capture {
                break;
            }
        }
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

        let player_id = app.world.spawn().insert(Player { score: 0 }).id();

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
        app.add_system(update_scores);
        app.add_system(perform_selection.before(update_scores));

        let mut events = app.world.resource_mut::<Events<SelectEvent>>();
        events.send(SelectEvent {
            player: player_id,
            row: 0,
            column: 0,
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
