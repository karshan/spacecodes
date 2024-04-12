use std::f32::consts::PI;
use raylib::prelude::*;
use sc_types::{constants::{message_color, ship, ship_color, station}, GameState};

use crate::MouseState;

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

fn draw_border(img: &mut Image, c: Color) {
    for i in 0..2048 {
        for j in 0..32 {
            img.draw_pixel(i, j, c);
            img.draw_pixel(j, i, c);
        }
    }
}

fn vec3(v2: Vector2, z: f32) -> Vector3 {
    Vector3::new(v2.x, v2.y, z)
}

pub struct Renderer {
    shader: Shader,
    lights: Vec<Light>,
    background_color: Color,
    floor: Model,
    cube: Model,
    xtr: Texture2D,
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
        let emissive_intensity_loc = shader.get_shader_location("emissivePower");
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
    
        let background_color = Color::from_hex("264653").unwrap();
    
        let mut img = Image::load_image("sc-client/assets/tex4.png").unwrap();
        draw_border(&mut img, background_color);
        let xtr_tile = rl.load_texture_from_image(&thread, &mut img).unwrap();
        
        let w = 1.0;
        let h = 1.0;
        let mut floor = rl.load_model_from_mesh(&thread, unsafe { Mesh::gen_mesh_plane(&thread, w, h, 4, 3).make_weak() }).unwrap();
        floor.materials_mut()[0].set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &xtr_tile);
        floor.materials_mut()[0].shader = shader.clone();
        floor.set_transform(&(Matrix::translate(0.0, 0.0, 0.0) * Matrix::rotate_x(PI/2.0)));
    
        let cube_pos_loc = shader.get_shader_location("cubePos");
        let cube_size = Vector3::new(0.5, 0.5, 0.5);
        shader.set_shader_value(shader.get_shader_location("cubeSize"), cube_size);
        let mut cube = rl.load_model_from_mesh(&thread, unsafe { Mesh::gen_mesh_cube(&thread, cube_size.x, cube_size.y, cube_size.z).make_weak() }).unwrap();
        cube.materials_mut()[0].shader = shader.clone();
    
        let mut ctr = 0;
        let mut cube_pos = Vector3::new(0.0, 0.0, 0.5);
        let mut cube_dir = Vector3::new(1.0, 0.0, 0.0);
        
        shader.set_shader_value(shader.get_shader_location("useTexNormal"), 0);
        shader.set_shader_value(shader.get_shader_location("useTexMRA"), 0);
        shader.set_shader_value(shader.get_shader_location("useTexEmissive"), 0);
        Renderer {
            shader: shader,
            lights: lights,
            background_color: background_color,
            floor: floor,
            cube: cube,
            xtr: xtr_tile,
        }
    }

    pub fn iso_proj(screen_width: f64, screen_height: f64) -> Matrix {
        let aspect = screen_width/screen_height;
    
        let clip = 14f64;
        Matrix::ortho(-clip * aspect, clip * aspect, -clip, clip, -clip, clip) *
            Matrix::rotate_x(-35.264 * 2.0 * PI/360.0) * Matrix::rotate_z(PI/4.0)
    }
    
    pub fn screen2world(raw_mouse_position: Vector2, screen_width: f64, screen_height: f64) -> Vector3 {
        let screen2world_mat = Renderer::iso_proj(screen_width, screen_height).inverted() *
        Matrix::translate(-1.0, 1.0, 0.0) *
        Matrix::scale(2.0/screen_width as f32, -2.0/screen_height as f32, 1.0);
        let mut mouse_position = Vector3::new(raw_mouse_position.x, raw_mouse_position.y, 0f32).transform_with(screen2world_mat);
        mouse_position.x += mouse_position.z/2.0;
        mouse_position.y += mouse_position.z/2.0;
        mouse_position.z = 0.0;
        mouse_position
    }
    
    pub fn render(self: &mut Renderer, rl: &mut RaylibHandle, thread: &RaylibThread, frame_counter: i64, game_state: &GameState, mouse_position: Vector2, mouse_state: &MouseState) {
        let ctr = frame_counter;
        let screen_width = rl.get_screen_width() as f64;
        let screen_height = rl.get_screen_height() as f64;
    
        self.shader.set_shader_value(self.shader.locs()[ShaderLocationIndex::SHADER_LOC_VECTOR_VIEW as usize], Vector3::new(0.0, 0.0, 2.0f32.sqrt()));
        for i in 0..4 {
            update_light(&mut self.shader, &self.lights[i])
        }
    
        let mut _d = rl.begin_drawing(&thread);
        let mut _3d = _d.begin_mode3D(Camera3D::orthographic(Vector3::new(2.0, 2.0, 6.0), Vector3::zero(), Vector3::new(0.0, 1.0, 0.0), 45.0));
        _3d.set_matrix_modelview(&thread, Renderer::iso_proj(screen_width, screen_height));
        _3d.set_matrix_projection(&thread, Matrix::identity());
        
        _3d.clear_background(self.background_color);
    
        self.shader.set_shader_value(self.shader.get_shader_location("useTexAlbedo"), 1);
        self.shader.set_shader_value(self.shader.get_shader_location("useAo"), 1);
        for x in -12..12 {
            for y in -12..12 {
                let v = Vector2::new(x as f32, y as f32);
                let mut c = Color::from_hex("d9d9d9").unwrap();
                let mut reset_tex = false;
                if (mouse_position.x.round() == x as f32 && mouse_position.y.round() == y as f32) {
                    if *ship(0) == v {
                        reset_tex = true;
                        self.shader.set_shader_value(self.shader.get_shader_location("useTexAlbedo"), 0);
                        c = ship_color(0);
                    } else if *ship(1) == v {
                        reset_tex = true;
                        self.shader.set_shader_value(self.shader.get_shader_location("useTexAlbedo"), 0);
                        c = ship_color(1);
                    } else {
                        c = Color::WHITE;
                    }
                } else {
                    if *ship(0) == v {
                        c = ship_color(0).alpha(0.5);
                    } else if *ship(1) == v {
                        c = ship_color(1).alpha(0.5);
                    }
                }
                self.floor.set_transform(&(Matrix::translate(x as f32, y as f32, 0.0) * Matrix::rotate_x(PI/2.0)));
                _3d.draw_model(&self.floor, Vector3::zero(), 1.0, c);
                if reset_tex {
                    self.shader.set_shader_value(self.shader.get_shader_location("useTexAlbedo"), 1);
                }
            }
        }
        self.shader.set_shader_value(self.shader.get_shader_location("useTexAlbedo"), 0);
        self.shader.set_shader_value(self.shader.get_shader_location("useAo"), 0);

        _3d.draw_cube(vec3(*station(0), 0.25), 0.1, 0.1, 0.5, ship_color(0).alpha(0.5));
        _3d.draw_cube(vec3(*station(1), 0.25), 0.1, 0.1, 0.5, ship_color(1).alpha(0.5));

        
        // TODO get from game_state
        let cubes = game_state.my_units.iter().chain(game_state.other_units.iter()).map(|u| vec3(u.pos, 0.5)).collect::<Vec<Vector3>>();
        // let cubes = [vec3(game_state.my_units.len(), ), Vector3::new(3.0, 3.0, 0.5)];
        if (cubes.len() <= 20) {
            self.shader.set_shader_value_v(self.shader.get_shader_location("cubePos"), cubes.as_slice());
            self.shader.set_shader_value(self.shader.get_shader_location("numCubes"), cubes.len() as i32);
        } else {
            self.shader.set_shader_value(self.shader.get_shader_location("numCubes"), 0);
        }
        for u in game_state.my_units.iter().chain(game_state.other_units.iter()) {
            // Color::from_hex("83c5be").unwrap()
            _3d.draw_model(&self.cube, vec3(u.pos, 0.5), 1.0, message_color(u.player_id));
        }

        
        match mouse_state {
            MouseState::Path(path, y_first) => {
                let mut p = path[0];
                for i in 1..path.len() {
                    let next_p = path[i];
                    _3d.draw_line_3D(vec3(p, 0.01), vec3(next_p, 0.01), Color::WHITE);
                    p = next_p;
                }
                let m: Vector3;
                if *y_first {
                    m = Vector3::new(p.x.round(), mouse_position.y.round(), 0.01);
                } else {
                    m = Vector3::new(mouse_position.x.round(), p.y.round(), 0.01);
                }
                _3d.draw_line_3D(vec3(p, 0.01), m, Color::WHITE);
                _3d.draw_line_3D(m, Vector3::new(mouse_position.x.round(), mouse_position.y.round(), 0.01), Color::WHITE);
            }
            _ => {}
        }
    }
}

