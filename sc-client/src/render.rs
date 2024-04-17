use std::{collections::HashMap, f32::consts::PI};
use raylib::prelude::*;
use sc_types::*;
use sc_types::constants::*;
use serde_json::Value;

use crate::{rounded, scale_color, vec2, vec3, ClientState, Interception, MouseState, NetInfo};

#[derive(Clone, Copy)]
#[repr(C)]
pub enum LightType {
    LIGHT_DIRECTIONAL = 0,
    LIGHT_POINT
}

#[repr(C)]
pub struct Light {
    _type: LightType,
    enabled: i32,
    position: Vector3,
    target: Vector3,
    color: Vector4,
    intensity: f32,

    type_loc: i32,
    enabled_loc: i32,
    position_loc: i32,
    target_loc: i32,
    color_loc: i32,
    intensity_loc: i32
}

fn update_light(shader: &mut Shader, light: &Light) {
    shader.set_shader_value(light.enabled_loc, light.enabled);
    shader.set_shader_value(light.type_loc, light._type as i32);
    
    shader.set_shader_value(light.position_loc, light.position);

    shader.set_shader_value(light.target_loc, light.target);
    shader.set_shader_value(light.color_loc, light.color);
    shader.set_shader_value(light.intensity_loc, light.intensity);
}

fn create_light(_type: LightType, position: Vector3, target: Vector3, color: Color, intensity: f32, shader: &mut Shader, light_idx: usize) -> Light {
    let light = Light {
        enabled: 1,
        _type: _type,
        position: position,
        target: target,
        color: color.color_normalize(),
        intensity: intensity,
        enabled_loc: shader.get_shader_location(&format!("lights[{}].enabled", light_idx)),
        type_loc: shader.get_shader_location(&format!("lights[{}].type", light_idx)),
        position_loc: shader.get_shader_location(&format!("lights[{}].position", light_idx)),
        target_loc: shader.get_shader_location(&format!("lights[{}].target", light_idx)),
        color_loc: shader.get_shader_location(&format!("lights[{}].color", light_idx)),
        intensity_loc: shader.get_shader_location(&format!("lights[{}].intensity", light_idx)),
    };

    update_light(shader, &light);
    light
}

fn draw_border(img: &mut Image, c: Color, thickness: i32) {
    for i in 0..2048 {
        for j in 0..thickness {
            img.draw_pixel(i, j, c);
            img.draw_pixel(j, i, c);
        }
    }
}

struct Constants(Value);

impl Constants {
    fn get_p_color(self: &Self, s: &str, idx: usize) -> Color {
        let hex: &str = (|| {
            self.0.get(s)?.as_array()?.get(idx)?.as_str()
        })().unwrap_or("000000");
        Color::from_hex(hex).unwrap_or(Color::BLACK)
    }

    fn get_color(self: &Self, s: &str) -> Color {
        let hex: &str = (|| {
            self.0.get(s)?.as_str()
        })().unwrap_or("000000");
        Color::from_hex(hex).unwrap_or(Color::BLACK)
    }

    fn get_f32(self: &Self, s: &str) -> f32 {
        (|| {
            self.0.get(s)?.as_f64()
        })().unwrap_or(0.0) as f32
    }

    fn get_i32(self: &Self, s: &str) -> i32 {
        (|| {
            self.0.get(s)?.as_i64()
        })().unwrap_or(0) as i32
    }

    fn get_vec3(self: &Self, s: &str) -> Vector3 {
        (|| {
            let x = self.0.get(s)?.as_array()?.get(0)?.as_f64()?;
            let y = self.0.get(s)?.as_array()?.get(1)?.as_f64()?;
            let z = self.0.get(s)?.as_array()?.get(2)?.as_f64()?;
            Some(Vector3::new(x as f32, y as f32, z as f32))
        })().unwrap_or(Vector3::zero())
    }
}

pub struct ShaderLocs {
    use_ao: i32,
    use_tex_albedo: i32,
    use_tex_emissive: i32,
    num_cubes: i32,
    cube_pos: i32,
    cube_size: i32,
    gcube_pos: i32,
    gcube_size: i32,
    use_hdr_tone_map: i32,
    use_gamma: i32,
    light_mult: i32,
    emissive_power: i32,
    emissive_color: i32,
    ao_intensity: i32,
    ao_stepsize: i32,
    ao_iterations: i32,
    shadow_mint: i32,
    shadow_maxt: i32,
    shadow_w: i32,
    shadow_intensity: i32,
    shadow_light: i32,
    bounty_pos: i32,
    bounty_r: i32,
    num_bounties: i32
}

pub struct Renderer {
    cs: Constants,
    shader: Shader,
    lights: Vec<Light>,
    background_color: Color,
    floor: Model,
    plane: Model,
    sphere: Model,
    xtr_sky: Texture2D,
    locs: ShaderLocs,
}

impl Renderer {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Renderer {
        let mut shader = rl.load_shader(&thread, Some("sc-client/src/pbr.vert"), Some("sc-client/src/pbr.frag")).unwrap();
    
        shader.locs_mut()[ShaderLocationIndex::SHADER_LOC_MAP_ALBEDO as usize] = shader.get_shader_location("albedoMap");
        shader.locs_mut()[ShaderLocationIndex::SHADER_LOC_MAP_NORMAL as usize] = shader.get_shader_location("normalMap");
        shader.locs_mut()[ShaderLocationIndex::SHADER_LOC_MAP_EMISSION as usize] = shader.get_shader_location("emissiveMap");
        shader.locs_mut()[ShaderLocationIndex::SHADER_LOC_COLOR_DIFFUSE as usize] = shader.get_shader_location("albedoColor");
        shader.locs_mut()[ShaderLocationIndex::SHADER_LOC_VECTOR_VIEW as usize] = shader.get_shader_location("viewPos");
        let light_count_loc = shader.get_shader_location("numOfLights");
        shader.set_shader_value(light_count_loc, 4);
    
        // Get location for shader parameters that can be modified in real time
        let emissive_power_loc = shader.get_shader_location("emissivePower");
        let emissive_color_loc = shader.get_shader_location("emissiveColor");
        shader.set_shader_value(shader.get_shader_location("tiling"), Vector2::new(0.5, 0.5));
    
        let mut lights: Vec<Light> = vec![];
        lights.push(create_light(LightType::LIGHT_DIRECTIONAL, Vector3::new(-0.5, -0.6, 0.4), Vector3::new(0.0, 0.0, 0.0), Color::WHITE, 6.5, &mut shader, 0));
        lights.push(create_light(LightType::LIGHT_POINT, Vector3::new(5.0, -5.0, 5.0), Vector3::new(0.0, 0.0, 0.0), Color::WHITE, 3.3, &mut shader, 1));
        lights.push(create_light(LightType::LIGHT_POINT, Vector3::new(-5.0, 5.0, 5.0), Vector3::new(0.0, 0.0, 0.0), Color::WHITE, 8.3, &mut shader, 2));
        lights.push(create_light(LightType::LIGHT_POINT, Vector3::new(5.0, 5.0, 5.0), Vector3::new(0.0, 0.0, 0.0), Color::WHITE, 2.0, &mut shader, 3));
    
        lights[1].enabled = 0;
        lights[2].enabled = 0;
        lights[3].enabled = 0;

        for i in 0..4 {
            update_light(&mut shader, &lights[i])
        }
    
        let background_color = Color::from_hex("264653").unwrap();
    
        let constants = Constants(serde_json::from_str(&std::fs::read_to_string("constants.json").unwrap()).unwrap_or(Value::Null));

        let mut sky = Image::load_image("sc-client/assets/sky.png").unwrap();
        sky.resize(rl.get_screen_width(), rl.get_screen_height());
        let xtr_sky = rl.load_texture_from_image(&thread, &mut sky).unwrap();
        
        let w = 1.0;
        let h = 1.0;
        let mut floor = rl.load_model_from_mesh(&thread, unsafe { Mesh::gen_mesh_plane(&thread, w, h, 1, 1).make_weak() }).unwrap();
        floor.materials_mut()[0].shader = shader.clone();
        floor.set_transform(&(Matrix::translate(0.0, 0.0, 0.0) * Matrix::rotate_x(PI/2.0)));

        let mut plane = rl.load_model_from_mesh(&thread, unsafe { Mesh::gen_mesh_plane(&thread, 1.0, 1.0, 4, 3).make_weak() }).unwrap();
        plane.materials_mut()[0].shader = shader.clone();
        plane.set_transform(&(Matrix::translate(0.0, 0.0, 0.0) * Matrix::rotate_x(PI/2.0)));

        let mut sphere = rl.load_model_from_mesh(&thread, unsafe { Mesh::gen_mesh_sphere(&thread, 1.0, 32, 32).make_weak() }).unwrap();
        sphere.materials_mut()[0].shader = shader.clone();
    
        let mut ctr = 0;
        let mut cube_pos = Vector3::new(0.0, 0.0, 0.5);
        let mut cube_dir = Vector3::new(1.0, 0.0, 0.0);
        
        shader.set_shader_value(shader.get_shader_location("useTexNormal"), 0);
        shader.set_shader_value(shader.get_shader_location("useTexMRA"), 0);
        shader.set_shader_value(shader.get_shader_location("useTexEmissive"), 0);
        shader.set_shader_value(shader.get_shader_location("useTexAlbedo"), 0);
        shader.set_shader_value(shader.get_shader_location("useAo"), 1);
        shader.set_shader_value(shader.locs()[ShaderLocationIndex::SHADER_LOC_VECTOR_VIEW as usize], Vector3::new(0.0, 0.0, 2.0f32.sqrt()));
        Renderer {
            cs: constants,
            lights: lights,
            background_color: background_color,
            floor: floor,
            plane: plane,
            sphere: sphere,
            xtr_sky,
            locs: ShaderLocs {
                use_tex_albedo: shader.get_shader_location("useTexAlbedo"),
                use_tex_emissive: shader.get_shader_location("useTexEmissive"),
                use_ao: shader.get_shader_location("useAo"),
                cube_pos: shader.get_shader_location("cubePos"),
                cube_size: shader.get_shader_location("cubeSize"),
                gcube_pos: shader.get_shader_location("gcubePos"),
                gcube_size: shader.get_shader_location("gcubeSize"),
                num_cubes: shader.get_shader_location("numCubes"),
                bounty_pos: shader.get_shader_location("bountyPos"),
                bounty_r: shader.get_shader_location("bountyR"),
                num_bounties: shader.get_shader_location("numBounties"),
                use_hdr_tone_map: shader.get_shader_location("useHdrToneMap"),
                use_gamma: shader.get_shader_location("useGamma"),
                light_mult: shader.get_shader_location("lightMult"),
                emissive_color: emissive_color_loc,
                emissive_power: emissive_power_loc,
                ao_intensity: shader.get_shader_location("ao_intensity"),
                ao_stepsize: shader.get_shader_location("ao_stepsize"),
                ao_iterations: shader.get_shader_location("ao_iterations"),
                shadow_mint: shader.get_shader_location("shadow_mint"),
                shadow_maxt: shader.get_shader_location("shadow_maxt"),
                shadow_w: shader.get_shader_location("shadow_w"),
                shadow_intensity: shader.get_shader_location("shadow_intensity"),
                shadow_light: shader.get_shader_location("shadow_light"),
            },
            shader: shader,
        }
    }

    pub fn iso_proj(screen_width: f64, screen_height: f64, zoom: bool) -> Matrix {
        let aspect = screen_width/screen_height;
    
        let clip = if zoom { 10f64 } else { 19f64 };
        Matrix::ortho(-clip, clip, -clip / aspect, clip / aspect, -clip, clip) *
            Matrix::rotate_x(-(1.0/3f32.sqrt()).acos()) * Matrix::rotate_z(PI/4.0)
    }
    
    pub fn screen2world(raw_mouse_position: Vector2, screen_width: f64, screen_height: f64, zoom: bool) -> Vector3 {
        let screen2world_mat = Renderer::iso_proj(screen_width, screen_height, zoom).inverted() *
        Matrix::translate(-1.0, 1.0, 0.0) *
        Matrix::scale(2.0/screen_width as f32, -2.0/screen_height as f32, 1.0);
        let mut mouse_position = Vector3::new(raw_mouse_position.x, raw_mouse_position.y, 0f32).transform_with(screen2world_mat);
        mouse_position.x += mouse_position.z;
        mouse_position.y += mouse_position.z;
        mouse_position.z = 0.0;
        mouse_position
    }

    pub fn screen2clip(raw_mouse_position: Vector2, screen_width: f64, screen_height: f64) -> Vector2 {
        let screen2clip_mat = Matrix::translate(-1.0, 1.0, 0.0) *
            Matrix::scale(2.0/screen_width as f32, -2.0/screen_height as f32, 1.0);
        let mut mouse_position = Vector3::new(raw_mouse_position.x, raw_mouse_position.y, 0f32).transform_with(screen2clip_mat);
        mouse_position.x += mouse_position.z;
        mouse_position.y += mouse_position.z;
        mouse_position.z = 0.0;
        Vector2::new(mouse_position.x, mouse_position.y)
    }

    fn draw_cube_outline<'a>(self: &mut Self, _3d: &mut RaylibMode3D<'a, RaylibDrawHandle>, pos: Vector3, cube_size: f32, cube_z_offset: f32, highlight_color: Color, thickness: f32) {
        let z_thickness = thickness;
        let bring_front = Matrix::translate(-0.01, -0.01, 0.01);
        self.plane.set_transform(&(bring_front * Matrix::translate(0.0, cube_size/2.0 + thickness/2.0, cube_z_offset + cube_size/2.0) * Matrix::scale(cube_size, thickness, 1.0) * Matrix::rotate_x(PI/2.0)));
        _3d.draw_model(&self.plane, pos, 1.0, highlight_color);
        self.plane.set_transform(&(bring_front * Matrix::translate(cube_size/2.0 + thickness/2.0, thickness/2.0, cube_z_offset + cube_size/2.0) * Matrix::scale(thickness, cube_size + thickness, 1.0) * Matrix::rotate_x(PI/2.0)));
        _3d.draw_model(&self.plane, pos, 1.0, highlight_color);

        self.plane.set_transform(&(bring_front * Matrix::translate(-cube_size/2.0, cube_size/2.0 + thickness/2.0, cube_z_offset) * Matrix::scale(1.0, thickness, cube_size) * Matrix::rotate_y(-PI/2.0) * Matrix::rotate_x(PI/2.0)));
        _3d.draw_model(&self.plane, pos, 1.0, highlight_color);

        self.plane.set_transform(&(bring_front * Matrix::translate(cube_size/2.0 + thickness/2.0, -cube_size/2.0, cube_z_offset) * Matrix::scale(thickness, 1.0, cube_size) * Matrix::rotate_z(PI/2.0) * Matrix::rotate_y(-PI/2.0) * Matrix::rotate_x(PI/2.0)));
        _3d.draw_model(&self.plane, pos, 1.0, highlight_color);

        self.plane.set_transform(&(bring_front * Matrix::translate(0.0 + thickness/2.0, -cube_size/2.0, cube_z_offset - cube_size/2.0 - z_thickness/2.0) * Matrix::scale(cube_size + thickness, 1.0, z_thickness) * Matrix::rotate_z(PI/2.0) * Matrix::rotate_y(-PI/2.0) * Matrix::rotate_x(PI/2.0)));
        _3d.draw_model(&self.plane, pos, 1.0, highlight_color);

        self.plane.set_transform(&(bring_front * Matrix::translate(-cube_size/2.0, thickness/2.0, cube_z_offset - cube_size/2.0 - z_thickness/2.0) * Matrix::scale(cube_size, cube_size + thickness, z_thickness) * Matrix::rotate_y(-PI/2.0) * Matrix::rotate_x(PI/2.0)));
        _3d.draw_model(&self.plane, pos, 1.0, highlight_color);
    }

    pub fn render_map(self: &mut Renderer, _3d: &mut RaylibMode3D<RaylibDrawHandle>, mouse_position: Vector3, interceptions: &Vec<Interception>, frame_counter: i64) {
        let mut tile_color = HashMap::new();
        for i in interceptions {
            let alpha = ((frame_counter - i.start_frame) as f32/INTERCEPT_DELAY as f32).min(1.0);
            let c = self.cs.get_color("tile_tint").color_normalize().lerp(self.cs.get_p_color("message_color", i.player_id).color_normalize(), alpha);
            let e = self.cs.get_color("e_color").color_normalize().lerp(self.cs.get_p_color("message_emission", i.player_id).color_normalize(), alpha);
            let end_ep = self.cs.get_f32(&format!("message_e_power{}", i.player_id));
            let start_ep = self.cs.get_f32("e_power");
            let ep = start_ep + (end_ep - start_ep)*alpha;
            
            tile_color.insert((i.pos.x as i32, i.pos.y as i32), 
                (Color::color_from_normalized(c),
                e,
                ep));
        }

        for s in station(0) {
            tile_color.insert((s.x as i32, s.y as i32), (self.cs.get_p_color("message_color", 0),
                self.cs.get_p_color("message_emission", 0).color_normalize(),
                self.cs.get_f32(&format!("message_e_power{}", 0))));
        }

        for s in station(1) {
            tile_color.insert((s.x as i32, s.y as i32), (self.cs.get_p_color("message_color", 1),
                self.cs.get_p_color("message_emission", 1).color_normalize(),
                self.cs.get_f32(&format!("message_e_power{}", 1))));
        }

        let rounded_mpos = rounded(vec2(mouse_position));
        if PLAY_AREA.contains_point(&rounded_mpos) {
            tile_color.entry((rounded_mpos.x as i32, rounded_mpos.y as i32)).and_modify(|e| *e = (scale_color(e.0, self.cs.get_f32("highlight_mult")), e.1, e.2));
            tile_color.entry((rounded_mpos.x as i32, rounded_mpos.y as i32)).or_insert(
                (scale_color(self.cs.get_color("tile_tint"), self.cs.get_f32("highlight_mult")), self.cs.get_color("e_color").color_normalize(), self.cs.get_f32("e_power")));
        }

        self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32("e_power"));
        self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_color("e_color").color_normalize());
        self.shader.set_shader_value(self.locs.use_tex_albedo, 1);
        // PERF 1 plane 2 triangles
        for x in -12..=12 {
            for y in -12..=12 {
                let mut c = Color::WHITE; //self.cs.get_color("tile_tint");
                let mut reset_emissive = false;
                if let Some((overwrite_c, e, ep)) = tile_color.get(&(x, y)) {
                    c = *overwrite_c;
                    self.shader.set_shader_value(self.locs.emissive_power, *ep);
                    self.shader.set_shader_value(self.locs.emissive_color, *e);
                    self.shader.set_shader_value(self.locs.use_tex_albedo, 0);
                    reset_emissive = true;
                }
                self.floor.set_transform(&(Matrix::translate(x as f32, y as f32, 0.0) * Matrix::rotate_x(PI/2.0)));
                _3d.draw_model(&self.floor, Vector3::zero(), 1.0, c);

                if reset_emissive {
                    self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32("e_power"));
                    self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_color("e_color").color_normalize());
                    self.shader.set_shader_value(self.locs.use_tex_albedo, 1);
                }
            }
        }
        self.shader.set_shader_value(self.locs.use_tex_albedo, 0);
        self.shader.set_shader_value(self.locs.use_tex_emissive, 0);

        self.plane.set_transform(&(Matrix::rotate_x(PI)));
        for x in -12..=12 {
            _3d.draw_model(&self.plane, Vector3::new(x as f32, -12.5, -0.5), 1.0, self.cs.get_color("cliff"));
        }
        self.plane.set_transform(&(Matrix::rotate_z(PI/2.0)));
        for y in -12..=12 {
            _3d.draw_model(&self.plane, Vector3::new(-12.5, y as f32, -0.5), 1.0, self.cs.get_color("cliff"));
        }
        self.shader.set_shader_value(self.locs.emissive_power, 0f32);
    }
    
    fn render_ships(self: &mut Self, _3d: &mut RaylibMode3D<RaylibDrawHandle>, game_state: &GameState, cube_z_offset: f32, cube_side_len: f32, cube: &mut Model, p_id: usize) {
        self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32("message_e_power0"));
        self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_p_color("message_emission", 0).color_normalize());

        let alpha = |i| { (MSG_COOLDOWN - game_state.spawn_cooldown[i]) as f32/MSG_COOLDOWN as f32 };
        self.shader.set_shader_value_v(self.locs.gcube_pos, &[vec3(*ship(0), cube_z_offset), vec3(*ship(1), cube_z_offset)]);
        self.shader.set_shader_value_v(self.locs.gcube_size, &[alpha(0), alpha(1)]);

        cube.set_transform(&Matrix::scale(alpha(0), alpha(0), alpha(0)));
        _3d.draw_model(&cube, vec3(*ship(0), cube_z_offset), 1.0, self.cs.get_p_color("message_color", 0));

        self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32("message_e_power1"));
        self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_p_color("message_emission", 1).color_normalize());
        cube.set_transform(&Matrix::scale(alpha(1), alpha(1), alpha(1)));
        _3d.draw_model(&cube, vec3(*ship(1), cube_z_offset), 1.0, self.cs.get_p_color("message_color", 1));
    
        self.shader.set_shader_value(self.locs.emissive_power, 0f32);
        cube.set_transform(&Matrix::identity());

        self.lights[0].enabled = 0;
        update_light(&mut self.shader, &self.lights[0]);

        self.draw_cube_outline(_3d, vec3(*ship(p_id), 0.0), cube_side_len, cube_z_offset, self.cs.get_color("selection"), self.cs.get_f32("selection_thickness"));
        for u in game_state.selection.iter().filter(|s| if let Selection::Unit(_) = s { true } else { false }).map(|s| if let Selection::Unit(u) = s { game_state.my_units[*u].clone() } else { panic!("impossible") }) {
            self.draw_cube_outline(_3d, vec3(u.pos, 0.0), cube_side_len, cube_z_offset, self.cs.get_color("selection"), self.cs.get_f32("selection_thickness"));
        }

        self.lights[0].enabled = 1;
        update_light(&mut self.shader, &self.lights[0]);
    }

    fn render_messages(self: &mut Self, _3d: &mut RaylibMode3D<RaylibDrawHandle>, game_state: &GameState, cube_z_offset: f32, cube: &mut Model, p_id: usize) {
        let other_id = (p_id + 1) % 2;

        let cubes = game_state.my_units.iter().chain(game_state.other_units.iter()).map(|u| vec3(u.pos, cube_z_offset)).collect::<Vec<Vector3>>();
        if cubes.len() <= 20 {
            self.shader.set_shader_value_v(self.locs.cube_pos, cubes.as_slice());
            self.shader.set_shader_value(self.locs.num_cubes, cubes.len() as i32);
        } else {
            self.shader.set_shader_value(self.locs.num_cubes, 0);
        }
        self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32(&format!("message_e_power{}", p_id)));
        self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_p_color("message_emission", p_id).color_normalize());
        for u in game_state.my_units.iter() {
            _3d.draw_model(&cube, vec3(u.pos, cube_z_offset), 1.0, self.cs.get_p_color("message_color", u.player_id));
        }
        self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32(&format!("message_e_power{}", other_id)));
        self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_p_color("message_emission", other_id).color_normalize());
        for u in game_state.other_units.iter() {
            _3d.draw_model(&cube, vec3(u.pos, cube_z_offset), 1.0, self.cs.get_p_color("message_color", u.player_id));
        }

        self.shader.set_shader_value(self.locs.emissive_power, 0f32);

    }

    fn render_bounties(self: &mut Self, _3d: &mut RaylibMode3D<RaylibDrawHandle>, bounties: &Vec<Bounty>, frame_counter: i64) {
        let bounty_z = self.cs.get_f32("bounty_z") + self.cs.get_f32("bounty_hover_d") * ((frame_counter as f32/60f32) * self.cs.get_f32("bounty_hover_s")).sin();
        if bounties.len() <= 10 {
            self.shader.set_shader_value_v(self.locs.bounty_pos, bounties.iter().map(|b| vec3(b.pos, bounty_z)).collect::<Vec<_>>().as_slice());
            self.shader.set_shader_value(self.locs.num_bounties, bounties.len() as i32);
            self.shader.set_shader_value(self.locs.bounty_r, self.cs.get_f32("bounty_r"));
        } else {
            self.shader.set_shader_value(self.locs.num_bounties, 0);
        }
        for b in bounties.iter() {
            let k = match b.type_ {
                BountyEnum::Blink => "blink",
                BountyEnum::Fuel => "fuel",
                BountyEnum::Gold => "gold",
                BountyEnum::Lumber => "lumber",
            };
            _3d.draw_model(&self.sphere, vec3(b.pos, bounty_z), self.cs.get_f32("bounty_r"), self.cs.get_color(k));
        }
    }

    pub fn render(self: &mut Renderer, rl: &mut RaylibHandle, thread: &RaylibThread, frame_counter: i64, p_id: usize, game_state: &GameState,
            interceptions: &Vec<Interception>, mouse_position: Vector3, mouse_state: &MouseState, state: &ClientState, zoom: bool, net_info: &NetInfo) {
        self.cs = Constants(serde_json::from_str(&std::fs::read_to_string("constants.json").unwrap()).unwrap_or(Value::Null));

        let mut tile = Image::gen_image_color(256, 256, self.cs.get_color("tile_tint"));
        draw_border(&mut tile, scale_color(self.cs.get_color("tile_tint"), self.cs.get_f32("tile_border_mult")), self.cs.get_i32("tile_border_thickness"));
        let xtr_tile = rl.load_texture_from_image(&thread, &mut tile).unwrap();
        self.floor.materials_mut()[0].set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &xtr_tile);

        self.shader.set_shader_value(self.locs.ao_intensity, self.cs.get_f32("ao_intensity"));
        self.shader.set_shader_value(self.locs.ao_stepsize, self.cs.get_f32("ao_stepsize"));
        self.shader.set_shader_value(self.locs.ao_iterations, self.cs.get_i32("ao_iterations"));
        self.shader.set_shader_value(self.locs.shadow_mint, self.cs.get_f32("shadow_mint"));
        self.shader.set_shader_value(self.locs.shadow_maxt, self.cs.get_f32("shadow_maxt"));
        self.shader.set_shader_value(self.locs.shadow_w, self.cs.get_f32("shadow_w"));
        self.shader.set_shader_value(self.locs.shadow_intensity, self.cs.get_f32("shadow_intensity"));
        self.shader.set_shader_value(self.locs.shadow_light, self.cs.get_vec3("shadow_light"));

        self.lights[0].position = self.cs.get_vec3("light_pos");
        let lc = self.cs.get_vec3("light_color");
        self.lights[0].color = Vector4::new(lc.x, lc.y, lc.z, 1.0);
        update_light(&mut self.shader, &self.lights[0]);
        self.shader.set_shader_value(self.locs.use_hdr_tone_map, self.cs.get_i32("use_hdr_tone_map"));
        self.shader.set_shader_value(self.locs.use_hdr_tone_map, self.cs.get_i32("use_gamma"));
        self.shader.set_shader_value(self.locs.light_mult, self.cs.get_f32("light_mult"));

        self.shader.set_shader_value(self.locs.emissive_power, 0f32);

        let screen_width = rl.get_screen_width() as f64;
        let screen_height = rl.get_screen_height() as f64;
        let fps = rl.get_fps();
        let raw_mouse_position = rl.get_mouse_position();

        let cube_side_len = self.cs.get_f32("cube_size");
        let cube_size = Vector3::new(cube_side_len, cube_side_len, cube_side_len);
        let mut cube = rl.load_model_from_mesh(&thread, unsafe { Mesh::gen_mesh_cube(&thread, cube_size.x, cube_size.y, cube_size.z).make_weak() }).unwrap();
        cube.materials_mut()[0].shader = self.shader.clone();

        let mut _d = rl.begin_drawing(&thread);        
        _d.clear_background(self.background_color);
        _d.draw_texture(&self.xtr_sky, 0, 0, Color::WHITE);

        let mut _3d = _d.begin_mode3D(Camera3D::orthographic(Vector3::zero(), Vector3::zero(), Vector3::zero(), 0.0));
        _3d.set_matrix_projection(&thread, Matrix::identity());
        _3d.set_matrix_modelview(&thread, Renderer::iso_proj(screen_width, screen_height, zoom));

        self.render_map(&mut _3d, mouse_position, &interceptions, frame_counter);

        let cube_side_len = self.cs.get_f32("cube_size");
        let cube_size = Vector3::new(cube_side_len, cube_side_len, cube_side_len);
        self.shader.set_shader_value(self.locs.cube_size, cube_size);
        let cube_z_offset = self.cs.get_f32("cube_z_offset");

        self.render_ships(&mut _3d, game_state, cube_z_offset, cube_side_len, &mut cube, p_id);

        self.render_messages(&mut _3d, game_state, cube_z_offset, &mut cube, p_id);

        self.render_bounties(&mut _3d, &game_state.bounties, frame_counter);

        if let MouseState::Path(path, y_first) = mouse_state {
            let mut p = path[0];
            for i in 1..path.len() {
                let next_p = path[i];
                _3d.draw_line_3D(vec3(p, 0.01), vec3(next_p, 0.01), self.background_color);
                p = next_p;
            }
            let m: Vector3;
            if *y_first {
                m = Vector3::new(p.x.round(), mouse_position.y.round(), 0.01);
            } else {
                m = Vector3::new(mouse_position.x.round(), p.y.round(), 0.01);
            }
            _3d.draw_line_3D(vec3(p, 0.01), m, self.background_color);
            _3d.draw_line_3D(m, Vector3::new(mouse_position.x.round(), mouse_position.y.round(), 0.01), self.background_color);
        }
        if let MouseState::Intercept = mouse_state {
            let bring_front = rvec3(-0.01, -0.01, 0.01);
            let c = self.cs.get_p_color("message_color", p_id);
            self.shader.set_shader_value(self.locs.emissive_color, self.cs.get_p_color("message_emission", p_id).color_normalize());
            self.shader.set_shader_value(self.locs.emissive_power, self.cs.get_f32(&format!("message_e_power{}", p_id)));
            self.plane.set_transform(&Matrix::rotate_x(PI/2.0));
            _3d.draw_model(&self.plane, vec3(rounded(rvec2(mouse_position.x, mouse_position.y)), 0.0) + bring_front, 1.0, c);
        }

        drop(_3d);

        if let MouseState::Drag(start_pos) = mouse_state {
            let selection_pos = Vector2 { x: start_pos.x.min(raw_mouse_position.x), y: start_pos.y.min(raw_mouse_position.y) };
            let selection_size = Vector2 { x: (start_pos.x - raw_mouse_position.x).abs(), y: (start_pos.y - raw_mouse_position.y).abs() };
            _d.draw_rectangle_lines(selection_pos.x as i32, selection_pos.y as i32, selection_size.x as i32, selection_size.y as i32, Color::from_hex("00ff00").unwrap());
        }

        let text_size = screen_height as f32/50.0;
        let gap = Vector2::new(0.0, text_size + 10.0);
        let mut text_pos = Vector2::new(20.0, 0.0) + gap;

        _d.draw_text(&format!("{:?}", state), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);
        text_pos += gap;
        _d.draw_text(&format!("fps/g: {}/{}", fps, net_info.game_ps.get_hz().round()), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);
        text_pos += gap;
        _d.draw_text(&format!("w/1%/fd: {}/{}/{}", (net_info.waiting_avg.avg * 1000f64).round(), (net_info.waiting_avg.one_percent_max() * 1000f64).round(), net_info.my_frame_delay), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);

        text_pos = Vector2::new(20.0, screen_height as f32) - gap.scale_by(6.0);
        _d.draw_text(&format!("Gold: {}/{}", game_state.gold[p_id].round(), game_state.gold[(p_id + 1) % 2].round()), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);
        text_pos += gap;
        _d.draw_text(&format!("Lumber: {}/{}", game_state.lumber[p_id], game_state.lumber[(p_id + 1) % 2]), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);
        text_pos += gap;
        _d.draw_text(&format!("Fuel: {}/{}", (game_state.fuel[p_id] * 100)/START_FUEL, (game_state.fuel[(p_id + 1) % 2] * 100)/START_FUEL), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);
        text_pos += gap;
        _d.draw_text(&format!("K/D: {}/{}", game_state.intercepted[p_id], game_state.intercepted[(p_id + 1) % 2]), text_pos.x.round() as i32, text_pos.y.round() as i32, text_size.round() as i32, Color::WHITE);
    }
}

