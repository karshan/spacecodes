use petgraph::algo::astar;
use sc_types::shapes::*;
use sc_types::*;
use raylib::prelude::*;
use petgraph::{Graph, Undirected};

use crate::constants::*;

fn path_collides(rects: &[Rect<i32>], offsets: [Vector2; 4], pos: Vector2, target: Vector2) -> bool {
    let mut collided = false;
    for r in rects {
        for l in r.lines() {
            for o in offsets {
                if let Some(_) = raylib::check_collision_lines(pos + o, target + o, l[0], l[1]) {
                    collided = true;
                    break;
                }
            }
            if collided { break; }
        }
        if collided { break; }
    }
    collided
}

fn base_graph(unit_type: UnitEnum) -> Graph<Vector2, f32, Undirected> {
    let mut g = Graph::<Vector2, f32, Undirected>::new_undirected();
    let blocked = &GAME_MAP[0].1;
    let sp0 = &GAME_MAP[1].1;
    let st0 = &GAME_MAP[2].1;
    let sp1 = &GAME_MAP[3].1;
    let st1 = &GAME_MAP[4].1;
    /*
     *   n0--sp00--sp01--n3
     *    |              |
     *   st10           sp10
     *    |              |
     *   st11           sp11
     *    |              |
     *   n1--st00--st01--n2
     */
    let n0 = g.add_node(Vector2 { x: blocked.x as f32, y: blocked.y as f32 } - *unit_type.size() - Vector2::one());
    let n1 = g.add_node(Vector2 { x: blocked.x as f32, y: (blocked.y + blocked.h) as f32 } + Vector2 { x: -unit_type.size().x - 1f32, y: 1f32 });
    let n2 = g.add_node(Vector2 { x: (blocked.x + blocked.w) as f32, y: (blocked.y + blocked.h) as f32 } + Vector2::one());
    let n3 = g.add_node(Vector2 { x: (blocked.x + blocked.w) as f32, y: blocked.y as f32 } + Vector2 { x: 1f32, y: -unit_type.size().y - 1f32 });
    let sp00 = g.add_node(Vector2 { x: sp0.x as f32, y: sp0.y as f32 } + Vector2 { x: 1f32, y: -unit_type.size().y - 1f32 });
    let sp01 = g.add_node(Vector2 { x: (sp0.x + sp0.w) as f32, y: sp0.y as f32 } - *unit_type.size() - Vector2::one());
    let st00 = g.add_node(Vector2 { x: st0.x as f32, y: (st0.y + st0.h) as f32 } + Vector2::one());
    let st01 = g.add_node(Vector2 { x: (st0.x + st0.w) as f32, y: (st0.y + st0.h) as f32 } + Vector2 { x: -unit_type.size().x - 1f32, y: 1f32 });
    let sp10 = g.add_node(Vector2 { x: (sp1.x + sp1.w) as f32, y: sp1.y as f32 } + Vector2::one());
    let sp11 = g.add_node(Vector2 { x: (sp1.x + sp1.w) as f32, y: (sp1.y + sp1.h) as f32 } + Vector2 { x: 1f32, y: -unit_type.size().y - 1f32 });
    let st10 = g.add_node(Vector2 { x: st1.x as f32, y: st1.y as f32 } + Vector2 { x: -unit_type.size().x - 1f32, y: 1f32 });
    let st11 = g.add_node(Vector2 { x: st1.x as f32, y: (st1.y + st1.h) as f32 } - *unit_type.size() - Vector2::one());
    g.add_edge(n0, sp00, (g[n0] - g[sp00]).length());
    g.add_edge(sp00, sp01, (g[sp00] - g[sp01]).length());
    g.add_edge(sp01, n3, (g[sp01] - g[n3]).length());
    g.add_edge(n1, st00, (g[n1] - g[st00]).length());
    g.add_edge(st00, st01, (g[st00] - g[st01]).length());
    g.add_edge(st01, n2, (g[st01] - g[n2]).length());
    g.add_edge(n2, sp11, (g[n2] - g[sp11]).length());
    g.add_edge(n3, sp10, (g[n3] - g[sp10]).length());
    g.add_edge(sp10, sp11, (g[sp10] - g[sp11]).length());
    g.add_edge(n0, st10, (g[n0] - g[st10]).length());
    g.add_edge(st10, st11, (g[st10] - g[st11]).length());
    g.add_edge(st11, n1, (g[st11] - g[n1]).length());
    g
}

pub fn find_paths(units: &mut Vec<Unit>) {
    for u in units {
        if u.path.len() > 0 {
            continue;
        }

        if u.target_type == Target::Move {
            let target = u.target;
            if target == u.pos { continue; }
            let offsets = [ Vector2::zero(), *u.type_.size(), Vector2 { x: u.type_.size().x, y: 0f32 }, Vector2 { x: 0f32, y: u.type_.size().y } ];
            let rects = if u.player_id == 0 { &P0_BLOCKED } else { &P1_BLOCKED };
            
            if !path_collides(rects, offsets, u.pos, target) {
                u.path.push((target.x, target.y));
            } else {
                let mut g = base_graph(u.type_);
                let start_node = g.add_node(u.pos);
                for n in g.node_indices() {
                    if n == start_node { continue; }
                    if !path_collides(rects, offsets, u.pos, g[n]) {
                        g.add_edge(n, start_node, (g[n] - u.pos).length());
                    }
                }
                let end_node = g.add_node(target);
                for n in g.node_indices() {
                    if n == start_node || n == end_node { continue; }
                    if !path_collides(rects, offsets, target, g[n]) {
                        g.add_edge(n, end_node, (g[n] - target).length());
                    }
                }
                let path = astar(&g, start_node, |n| n == end_node, |e| *e.weight(), |n| (g[n] - target).length());
                match path {
                    None => {
                        u.path = vec![(target.x, target.y)]
                    },
                    Some(mut p) => {
                        p.1.reverse();
                        u.path = p.1.iter().map(|n| (g[*n].x, g[*n].y)).collect();
                    }
                }       
            }
        } else {
            continue;
        }        
    }
}