use crate::rng::{new_rng, SeededRng};
use crate::ship::{
    asteroid, cruiser, fighter, frigate, missile, target, ShipAccessor, ShipClass, ShipData,
    ShipHandle,
};
use crate::simulation::{Code, Line, Simulation, PHYSICS_TICK_LENGTH, WORLD_SIZE};
use crate::{bullet, collision, ship};
use bullet::BulletData;
use nalgebra::{vector, Point2, Rotation2, Vector2};
use rand::seq::SliceRandom;
use rand::Rng;
use rapier2d_f64::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use Status::Running;

pub const MAX_TICKS: u32 = 10000;

#[derive(PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Copy, Clone)]
pub enum Status {
    Running,
    Victory { team: i32 },
    Failed,
    Draw,
}

fn builtin(name: &str) -> Code {
    Code::Builtin(name.to_string())
}

fn reference_ai() -> Code {
    builtin("reference")
}

fn check_victory_with_filter(sim: &Simulation, ship_filter: fn(&ShipAccessor) -> bool) -> Status {
    let mut team_health: HashMap<i32, u32> = HashMap::new();
    for &handle in sim.ships.iter() {
        let ship = sim.ship(handle);
        if ship_filter(&ship) {
            *team_health.entry(ship.data().team).or_insert(0) += ship.data().health as u32;
        }
    }
    if team_health.is_empty() {
        Status::Draw
    } else if team_health.len() == 1 {
        Status::Victory {
            team: *team_health.iter().next().unwrap().0,
        }
    } else if sim.tick() >= MAX_TICKS - 1 {
        Status::Draw
    } else {
        Status::Running
    }
}

fn check_tutorial_victory(sim: &Simulation) -> Status {
    match check_victory_with_filter(sim, |ship| {
        ![ShipClass::Missile, ShipClass::Torpedo].contains(&ship.data().class)
    }) {
        x @ Status::Victory { team: 0 } => x,
        Status::Victory { .. } => Status::Failed,
        x => x,
    }
}

fn check_tournament_victory(sim: &Simulation) -> Status {
    check_victory_with_filter(sim, |ship| {
        [ShipClass::Fighter, ShipClass::Frigate, ShipClass::Cruiser].contains(&ship.data().class)
            && ship.data().team < 2
    })
}

fn check_capital_ship_tournament_victory(sim: &Simulation) -> Status {
    check_victory_with_filter(sim, |ship| {
        [ShipClass::Frigate, ShipClass::Cruiser].contains(&ship.data().class)
            && ship.data().team < 2
    })
}

fn fighter_without_missiles(team: i32) -> ShipData {
    let mut data = fighter(team);
    data.missile_launchers.pop();
    data
}

fn fighter_without_missiles_or_radar(team: i32) -> ShipData {
    let mut data = fighter(team);
    data.missile_launchers.pop();
    data.radar = None;
    data
}

fn target_asteroid(variant: i32) -> ShipData {
    let mut asteroid = asteroid(variant);
    asteroid.team = 1;
    asteroid
}

pub trait Scenario {
    fn name(&self) -> String;

    fn human_name(&self) -> String {
        self.name()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32);

    fn tick(&mut self, _: &mut Simulation) {}

    fn status(&self, _: &Simulation) -> Status {
        Running
    }

    // Indexed by team ID.
    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None]
    }

    fn solution(&self) -> Code {
        Code::None
    }

    fn solution_codes(&self) -> Vec<Code> {
        let mut codes = self.initial_code();
        codes[0] = self.solution();
        codes
    }

    fn next_scenario(&self) -> Option<String> {
        None
    }

    fn lines(&self) -> Vec<Line> {
        vec![]
    }

    fn is_tournament(&self) -> bool {
        false
    }
}

pub fn add_walls(sim: &mut Simulation) {
    let mut make_edge = |x: f64, y: f64, a: f64| {
        let edge_length = WORLD_SIZE as f64;
        let edge_width = 10.0;
        let rigid_body = RigidBodyBuilder::fixed()
            .translation(vector![x, y])
            .rotation(a)
            .build();
        let body_handle = sim.bodies.insert(rigid_body);
        let collider = ColliderBuilder::cuboid(edge_length / 2.0, edge_width / 2.0)
            .restitution(1.0)
            .collision_groups(collision::wall_interaction_groups())
            .build();
        sim.colliders
            .insert_with_parent(collider, body_handle, &mut sim.bodies);
    };
    make_edge(0.0, WORLD_SIZE / 2.0, 0.0);
    make_edge(0.0, -WORLD_SIZE / 2.0, std::f64::consts::PI);
    make_edge(WORLD_SIZE / 2.0, 0.0, std::f64::consts::PI / 2.0);
    make_edge(-WORLD_SIZE / 2.0, 0.0, 3.0 * std::f64::consts::PI / 2.0);
}

pub fn load_safe(name: &str) -> Option<Box<dyn Scenario>> {
    let scenario: Option<Box<dyn Scenario>> = match name {
        // Testing
        "test" => Some(Box::new(TestScenario {})),
        "basic" => Some(Box::new(BasicScenario {})),
        "gunnery" => Some(Box::new(GunneryScenario {})),
        "missile_test" => Some(Box::new(MissileTest::new())),
        "asteroid-stress" => Some(Box::new(AsteroidStressScenario {})),
        "bullet-stress" => Some(Box::new(BulletStressScenario {})),
        "missile-stress" => Some(Box::new(MissileStressScenario {})),
        "welcome" => Some(Box::new(WelcomeScenario::new())),
        "frigate_vs_cruiser" => Some(Box::new(FrigateVsCruiser::new())),
        "cruiser_vs_frigate" => Some(Box::new(CruiserVsFrigate::new())),
        "frigate_point_defense" => Some(Box::new(FrigatePointDefense {})),
        // Tutorials
        "tutorial01" => Some(Box::new(Tutorial01 {})),
        "tutorial02" => Some(Box::new(Tutorial02::new())),
        "tutorial03" => Some(Box::new(Tutorial03::new())),
        "tutorial04" => Some(Box::new(Tutorial04::new())),
        "tutorial05" => Some(Box::new(Tutorial05::new())),
        "tutorial06" => Some(Box::new(Tutorial06::new())),
        "tutorial07" => Some(Box::new(Tutorial07::new())),
        "tutorial08" => Some(Box::new(Tutorial08::new())),
        "tutorial09" => Some(Box::new(Tutorial09::new())),
        "tutorial10" => Some(Box::new(Tutorial10::new())),
        "tutorial11" => Some(Box::new(Tutorial11::new())),
        // Tournament
        "primitive_duel" => Some(Box::new(PrimitiveDuel::new())),
        "fighter_duel" => Some(Box::new(FighterDuel::new())),
        "frigate_duel" => Some(Box::new(FrigateDuel::new())),
        "cruiser_duel" => Some(Box::new(CruiserDuel::new())),
        "furball" => Some(Box::new(Furball::new())),
        "fleet" => Some(Box::new(Fleet::new())),
        "belt" => Some(Box::new(Belt::new())),
        _ => None,
    };
    if let Some(scenario) = scenario.as_ref() {
        assert_eq!(scenario.name(), name);
    }
    scenario
}

pub fn load(name: &str) -> Box<dyn Scenario> {
    match load_safe(name) {
        Some(scenario) => scenario,
        None => panic!("Unknown scenario"),
    }
}

pub fn list() -> Vec<String> {
    vec![
        "welcome",
        "tutorial01",
        "tutorial02",
        "tutorial03",
        "tutorial04",
        "tutorial05",
        "tutorial06",
        "tutorial07",
        "tutorial08",
        "tutorial09",
        "tutorial10",
        "tutorial11",
        "gunnery",
        "fighter_duel",
        "frigate_duel",
        "cruiser_duel",
        "furball",
        "fleet",
        "belt",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect()
}

struct TestScenario {}

impl Scenario for TestScenario {
    fn name(&self) -> String {
        "test".into()
    }

    fn init(&mut self, _sim: &mut Simulation, _seed: u32) {}
}

struct BasicScenario {}

impl Scenario for BasicScenario {
    fn name(&self) -> String {
        "basic".into()
    }

    fn init(&mut self, sim: &mut Simulation, _seed: u32) {
        add_walls(sim);
        ship::create(
            sim,
            vector![-100.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter(0),
        );
        ship::create(
            sim,
            vector![100.0, 0.0],
            vector![0.0, 0.0],
            std::f64::consts::PI,
            fighter(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }
}

struct GunneryScenario {}

impl Scenario for GunneryScenario {
    fn name(&self) -> String {
        "gunnery".into()
    }

    fn human_name(&self) -> String {
        "Gunnery".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        let mut ship_data = frigate(0);
        ship_data.guns.pop();
        ship_data.guns.pop();
        ship_data.missile_launchers.pop();
        ship_data.acceleration = vector![0.0, 0.0];
        ship::create(
            sim,
            vector![-9000.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            ship_data,
        );
        let mut rng = new_rng(seed);
        for _ in 0..4 {
            ship::create(
                sim,
                vector![
                    9000.0 + rng.gen_range(-500.0..500.0),
                    -9000.0 + rng.gen_range(-500.0..500.0)
                ],
                vector![
                    0.0 + rng.gen_range(-10.0..10.0),
                    700.0 + rng.gen_range(-300.0..600.0)
                ],
                std::f64::consts::PI,
                target(1),
            );
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn solution(&self) -> Code {
        builtin("gunnery")
    }
}

struct MissileTest {
    target: Option<ShipHandle>,
    rng: SeededRng,
    current_iteration: i64,
    tick_in_iteration: i64,
    acc: Vector2<f64>,
}

impl MissileTest {
    const MAX_ITERATIONS: i64 = 10;
    const MAX_ACCELERATION: f64 = 60.0;

    fn new() -> Self {
        Self {
            target: None,
            rng: new_rng(0),
            current_iteration: 0,
            tick_in_iteration: 0,
            acc: vector![0.0, 0.0],
        }
    }
}

impl Scenario for MissileTest {
    fn name(&self) -> String {
        "missile_test".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        log::info!("Running MissileTest iteration {}", self.current_iteration);
        let mut missile_data = missile(0);
        missile_data.ttl = None;

        self.rng = new_rng((seed % 1000) * 1000 + self.current_iteration as u32);
        let d = 4000.0;
        let target_p: Vector2<f64> = vector![self.rng.gen_range(-d..d), self.rng.gen_range(-d..d)];
        let s = 500.0;
        let target_v: Vector2<f64> = vector![self.rng.gen_range(-s..s), self.rng.gen_range(-s..s)];

        if let Some(radar) = missile_data.radar.as_mut() {
            radar.heading = target_p.angle(&vector![0.0, 0.0]);
            radar.width = TAU / 128.0;
        }

        ship::create(
            sim,
            vector![0.0, 0.0],
            vector![0.0, 0.0],
            target_p.y.atan2(target_p.x),
            missile_data,
        );
        let mut target_data = target(1);
        target_data.max_forward_acceleration = Self::MAX_ACCELERATION;
        target_data.max_backward_acceleration = Self::MAX_ACCELERATION;
        target_data.max_lateral_acceleration = Self::MAX_ACCELERATION;
        target_data.radar_cross_section = 1e6;
        self.target = Some(ship::create(
            sim,
            vector![target_p.x, target_p.y],
            vector![target_v.x, target_v.y],
            0.0,
            target_data,
        ));
    }

    fn tick(&mut self, sim: &mut Simulation) {
        let target = self.target.unwrap();
        if !sim.ships.contains(target) && self.current_iteration < MissileTest::MAX_ITERATIONS {
            self.current_iteration += 1;
            self.tick_in_iteration = 0;
            while !sim.bullets.is_empty() {
                let bullets: Vec<_> = sim.bullets.iter().cloned().collect();
                for handle in bullets {
                    sim.bullet_mut(handle).tick(PHYSICS_TICK_LENGTH);
                }
            }
            self.init(sim, 0);
        } else if sim.ships.contains(target) {
            if (self.tick_in_iteration % 60) == 0 {
                self.acc = Rotation2::new(self.rng.gen_range(0.0..std::f64::consts::TAU))
                    .transform_vector(&vector![Self::MAX_ACCELERATION, 0.0]);
            }
            sim.ship_mut(target).accelerate(self.acc);
        }
        self.tick_in_iteration += 1;
    }

    fn status(&self, sim: &Simulation) -> Status {
        if self.tick_in_iteration > 2000 {
            Status::Failed
        } else if sim.ships.contains(self.target.unwrap())
            || self.current_iteration < MissileTest::MAX_ITERATIONS
        {
            Status::Running
        } else {
            Status::Victory { team: 0 }
        }
    }

    fn solution(&self) -> Code {
        builtin("missile")
    }
}

struct AsteroidStressScenario {}

impl Scenario for AsteroidStressScenario {
    fn name(&self) -> String {
        "asteroid-stress".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        let mut rng = new_rng(seed);
        add_walls(sim);
        ship::create(sim, vector![0.0, 0.0], vector![0.0, 0.0], 0.0, fighter(0));

        let bound = (WORLD_SIZE / 2.0) * 0.9;
        for _ in 0..1000 {
            ship::create(
                sim,
                vector![rng.gen_range(-bound..bound), rng.gen_range(-bound..bound)],
                vector![rng.gen_range(-30.0..30.0), rng.gen_range(-30.0..30.0)],
                rng.gen_range(0.0..(2.0 * std::f64::consts::PI)),
                asteroid(rng.gen_range(0..30)),
            );
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }
}

struct BulletStressScenario {}

impl Scenario for BulletStressScenario {
    fn name(&self) -> String {
        "bullet-stress".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        let mut rng = new_rng(seed);
        add_walls(sim);
        ship::create(sim, vector![0.0, 0.0], vector![0.0, 0.0], 0.0, fighter(0));

        let bound = (WORLD_SIZE / 2.0) * 0.9;
        for _ in 0..1000 {
            let s = 1000.0;
            bullet::create(
                sim,
                vector![rng.gen_range(-bound..bound), rng.gen_range(-bound..bound)],
                vector![rng.gen_range(-s..s), rng.gen_range(-s..s)],
                BulletData {
                    mass: 0.1,
                    team: 0,
                    color: vector![1.00, 0.63, 0.00, 0.30],
                    ttl: 100.0,
                },
            );
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        if sim.bullets.is_empty() {
            Status::Victory { team: 0 }
        } else {
            Status::Running
        }
    }
}

struct MissileStressScenario {}

impl Scenario for MissileStressScenario {
    fn name(&self) -> String {
        "missile-stress".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        if seed != 0 {
            log::warn!("Ignoring nonzero seed {}", seed);
        }
        let mut rng = new_rng(0);
        add_walls(sim);

        let bound = (WORLD_SIZE / 2.0) * 0.9;
        for i in 0..100 {
            ship::create(
                sim,
                vector![rng.gen_range(-bound..bound), rng.gen_range(-bound..bound)],
                vector![rng.gen_range(-30.0..30.0), rng.gen_range(-30.0..30.0)],
                rng.gen_range(0.0..(2.0 * std::f64::consts::PI)),
                missile(i % 2),
            );
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        if sim.ships.len() < 50 {
            Status::Victory { team: 0 }
        } else {
            Status::Running
        }
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![reference_ai(), reference_ai()]
    }
}

struct WelcomeScenario {
    rng: Option<SeededRng>,
}

impl WelcomeScenario {
    fn new() -> Self {
        Self {
            rng: None as Option<SeededRng>,
        }
    }
}

impl Scenario for WelcomeScenario {
    fn name(&self) -> String {
        "welcome".into()
    }

    fn human_name(&self) -> String {
        "Welcome".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        self.rng = Some(new_rng(seed));
        let rng = self.rng.as_mut().unwrap();

        add_walls(sim);

        let ship_datas = &[fighter(0), frigate(0), cruiser(0)];
        let ship_data = rng.sample(rand::distributions::Slice::new(ship_datas).unwrap());
        ship::create(
            sim,
            vector![0.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            ship_data.clone(),
        );
    }

    fn tick(&mut self, sim: &mut Simulation) {
        let rng = self.rng.as_mut().unwrap();
        let asteroid_variants = [1, 6, 14];
        while sim.ships.len() < 20 {
            let p = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_point(&point![rng.gen_range(500.0..2000.0), 0.0]);
            ship::create(
                sim,
                vector![p.x, p.y],
                vector![rng.gen_range(-30.0..30.0), rng.gen_range(-30.0..30.0)],
                rng.gen_range(0.0..(2.0 * std::f64::consts::PI)),
                asteroid(*asteroid_variants.choose(rng).unwrap()),
            );
        }
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, Code::None]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }
}

struct Tutorial01 {}

impl Scenario for Tutorial01 {
    fn name(&self) -> String {
        "tutorial01".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 1: Guns".into()
    }

    fn init(&mut self, sim: &mut Simulation, _seed: u32) {
        add_walls(sim);
        ship::create(
            sim,
            vector![-1000.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles_or_radar(0),
        );
        ship::create(
            sim,
            vector![1000.0, 0.0],
            vector![0.0, 0.0],
            0.1,
            target_asteroid(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![builtin("tutorial/tutorial01.initial")]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial01.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial02".to_string())
    }
}

struct Tutorial02 {
    hit_target: bool,
}

impl Tutorial02 {
    const TARGET: Vector2<f64> = vector![1000.0, 0.0];

    fn new() -> Self {
        Self { hit_target: false }
    }
}

impl Scenario for Tutorial02 {
    fn name(&self) -> String {
        "tutorial02".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 2: Acceleration".into()
    }

    fn init(&mut self, sim: &mut Simulation, _seed: u32) {
        add_walls(sim);
        ship::create(
            sim,
            vector![-1000.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles_or_radar(0),
        );
        if let Some(&handle) = sim.ships.iter().next() {
            if let Some(c) = sim.ship_controllers.get_mut(&handle) {
                c.write_target(Self::TARGET);
            }
        }
    }

    fn tick(&mut self, sim: &mut Simulation) {
        if let Some(&handle) = sim.ships.iter().next() {
            let ship = sim.ship(handle);
            if (ship.position().vector - Self::TARGET).magnitude() < 50.0 {
                self.hit_target = true;
            }
        }
    }

    fn lines(&self) -> Vec<Line> {
        let mut lines = vec![];
        let center: Point2<f64> = Self::TARGET.into();
        let n = 20;
        let r = 50.0;
        let color = if self.hit_target {
            vector![0.0, 1.0, 0.0, 1.0]
        } else {
            vector![1.0, 0.0, 0.0, 1.0]
        };
        for i in 0..n {
            let frac = (i as f64) / (n as f64);
            let angle_a = std::f64::consts::TAU * frac;
            let angle_b = std::f64::consts::TAU * (frac + 1.0 / n as f64);
            lines.push(Line {
                a: center + vector![r * angle_a.cos(), r * angle_a.sin()],
                b: center + vector![r * angle_b.cos(), r * angle_b.sin()],
                color,
            });
        }
        lines
    }

    fn status(&self, _: &Simulation) -> Status {
        if self.hit_target {
            Status::Victory { team: 0 }
        } else {
            Status::Running
        }
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![builtin("tutorial/tutorial02.initial")]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial02.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial03".to_string())
    }
}

struct Tutorial03 {
    hit_target: bool,
    target: Option<Point2<f64>>,
}

impl Tutorial03 {
    fn new() -> Self {
        Self {
            hit_target: false,
            target: None,
        }
    }
}

impl Scenario for Tutorial03 {
    fn name(&self) -> String {
        "tutorial03".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 3: Acceleration #2".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        let mut rng = new_rng(seed);
        self.target = Some(
            Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_point(&point![rng.gen_range(100.0..500.0), 0.0]),
        );
        add_walls(sim);
        ship::create(
            sim,
            vector![0.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles_or_radar(0),
        );
        if let Some(&handle) = sim.ships.iter().next() {
            if let Some(c) = sim.ship_controllers.get_mut(&handle) {
                c.write_target(self.target.unwrap().coords);
            }
        }
    }

    fn tick(&mut self, sim: &mut Simulation) {
        if let Some(&handle) = sim.ships.iter().next() {
            let ship = sim.ship(handle);
            if (ship.position().vector - self.target.unwrap().coords).magnitude() < 50.0 {
                self.hit_target = true;
            }
        }
    }

    fn lines(&self) -> Vec<Line> {
        let mut lines = vec![];
        let center: Point2<f64> = self.target.unwrap();
        let n = 20;
        let r = 50.0;
        let color = if self.hit_target {
            vector![0.0, 1.0, 0.0, 1.0]
        } else {
            vector![1.0, 0.0, 0.0, 1.0]
        };
        for i in 0..n {
            let frac = (i as f64) / (n as f64);
            let angle_a = std::f64::consts::TAU * frac;
            let angle_b = std::f64::consts::TAU * (frac + 1.0 / n as f64);
            lines.push(Line {
                a: center + vector![r * angle_a.cos(), r * angle_a.sin()],
                b: center + vector![r * angle_b.cos(), r * angle_b.sin()],
                color,
            });
        }
        lines
    }

    fn status(&self, _: &Simulation) -> Status {
        if self.hit_target {
            Status::Victory { team: 0 }
        } else {
            Status::Running
        }
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![builtin("tutorial/tutorial03.initial")]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial03.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial04".to_string())
    }
}

struct Tutorial04 {}

impl Tutorial04 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial04 {
    fn name(&self) -> String {
        "tutorial04".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 4: Rotation".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        let mut rng = new_rng(seed);
        let target = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
            .transform_point(&point![rng.gen_range(100.0..500.0), 0.0]);
        ship::create(
            sim,
            vector![0.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles_or_radar(0),
        );
        if let Some(&handle) = sim.ships.iter().next() {
            if let Some(c) = sim.ship_controllers.get_mut(&handle) {
                c.write_target(target.coords);
            }
        }
        ship::create(
            sim,
            target.coords,
            vector![0.0, 0.0],
            0.0,
            target_asteroid(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![builtin("tutorial/tutorial04.initial")]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial04.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial05".to_string())
    }
}

struct Tutorial05 {
    ship_handle: Option<ShipHandle>,
    target_handle: Option<ShipHandle>,
}

impl Tutorial05 {
    fn new() -> Self {
        Self {
            ship_handle: None,
            target_handle: None,
        }
    }
}

impl Scenario for Tutorial05 {
    fn name(&self) -> String {
        "tutorial05".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 5: Deflection".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        self.ship_handle = Some(ship::create(
            sim,
            vector![0.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles_or_radar(0),
        ));

        let mut rng = new_rng(seed);
        let size = 500.0;
        let range = -size..size;
        let target = point![rng.gen_range(range.clone()), rng.gen_range(range)];
        self.target_handle = Some(ship::create(
            sim,
            target.coords,
            vector![
                rng.gen_range(0.0..std::f64::consts::TAU),
                rng.gen_range(-400.0..400.0)
            ],
            rng.gen_range(-400.0..400.0),
            fighter(1),
        ));

        if let Some(c) = sim.ship_controllers.get_mut(&self.ship_handle.unwrap()) {
            c.write_target(target.coords);
        }
    }

    fn tick(&mut self, sim: &mut Simulation) {
        if sim.ships.len() < 2 {
            return;
        }
        {
            let target_position = sim.ship(self.target_handle.unwrap()).position();
            if let Some(c) = sim.ship_controllers.get_mut(&self.ship_handle.unwrap()) {
                c.write_target(target_position.vector);
            }
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial05.initial"),
            builtin("tutorial/tutorial05.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial05.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial06".to_string())
    }
}

struct Tutorial06 {}

impl Tutorial06 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial06 {
    fn name(&self) -> String {
        "tutorial06".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 6: Radar".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        ship::create(
            sim,
            vector![0.0, 0.0],
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles(0),
        );

        let mut rng = new_rng(seed);
        let size = 500.0;
        let range = -size..size;
        for _ in 0..3 {
            let target = point![rng.gen_range(range.clone()), rng.gen_range(range.clone())];
            ship::create(
                sim,
                target.coords,
                vector![
                    rng.gen_range(0.0..std::f64::consts::TAU),
                    rng.gen_range(-400.0..400.0)
                ],
                rng.gen_range(-400.0..400.0),
                fighter(1),
            );
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial06.initial"),
            builtin("tutorial/tutorial06.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial06.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial07".to_string())
    }
}

struct Tutorial07 {}

impl Tutorial07 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial07 {
    fn name(&self) -> String {
        "tutorial07".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 7: Squadron".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut rng = new_rng(seed);
        for team in 0..2 {
            for _ in 0..10 {
                let size = 500.0;
                let range = -size..size;
                let center = vector![(team as f64 - 0.5) * 1000.0, 0.0];
                let offset = vector![rng.gen_range(range.clone()), rng.gen_range(range.clone())];
                let heading = if team == 0 { 0.0 } else { std::f64::consts::PI };
                ship::create(
                    sim,
                    center + offset,
                    vector![0.0, 0.0],
                    heading,
                    fighter(team),
                );
            }
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial07.initial"),
            builtin("tutorial/tutorial07.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial07.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial08".to_string())
    }
}

struct Tutorial08 {}

impl Tutorial08 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial08 {
    fn name(&self) -> String {
        "tutorial08".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 8: Search".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut rng = new_rng(seed);
        {
            for _ in 0..3 {
                let position = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                    .transform_point(&point![rng.gen_range(100.0..500.0), 0.0]);
                ship::create(
                    sim,
                    position.coords,
                    vector![0.0, 0.0],
                    rng.gen_range(0.0..std::f64::consts::TAU),
                    fighter_without_missiles(0),
                );
            }
        }
        {
            for _ in 0..3 {
                let position = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                    .transform_point(&point![rng.gen_range(3500.0..4500.0), 0.0]);
                ship::create(
                    sim,
                    position.coords,
                    vector![0.0, 0.0],
                    rng.gen_range(0.0..std::f64::consts::TAU),
                    fighter(1),
                );
            }
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial08.initial"),
            builtin("tutorial/tutorial08.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial08.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial09".to_string())
    }
}

struct Tutorial09 {}

impl Tutorial09 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial09 {
    fn name(&self) -> String {
        "tutorial09".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 9: Missiles".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut shipdata = fighter(0);
        shipdata.guns[0].cycle_time_remaining = 1e9;
        ship::create(sim, vector![0.0, 0.0], vector![0.0, 0.0], 0.0, shipdata);

        let mut rng = new_rng(seed);
        for _ in 0..3 {
            let p = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_vector(&vector![rng.gen_range(2000.0..2500.0), 0.0]);
            let v = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_vector(&vector![rng.gen_range(0.0..300.0), 0.0]);
            ship::create(sim, p, v, std::f64::consts::PI, fighter(1));
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial09.initial"),
            builtin("tutorial/tutorial09.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial09.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial10".to_string())
    }
}

struct Tutorial10 {}

impl Tutorial10 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial10 {
    fn name(&self) -> String {
        "tutorial10".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 10: Frigate".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        ship::create(sim, vector![0.0, 0.0], vector![0.0, 0.0], 0.0, frigate(0));

        let mut rng = new_rng(seed);
        for _ in 0..5 {
            let p = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_vector(&vector![rng.gen_range(1000.0..1500.0), 0.0]);
            let v = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_vector(&vector![rng.gen_range(0.0..300.0), 0.0]);
            ship::create(sim, p, v, std::f64::consts::PI, fighter(1));
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial10.initial"),
            builtin("tutorial/tutorial10.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial10.solution")
    }

    fn next_scenario(&self) -> Option<String> {
        Some("tutorial11".to_string())
    }
}

struct Tutorial11 {}

impl Tutorial11 {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Tutorial11 {
    fn name(&self) -> String {
        "tutorial11".into()
    }

    fn human_name(&self) -> String {
        "Tutorial 11: Cruiser".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        ship::create(sim, vector![0.0, 0.0], vector![0.0, 0.0], 0.0, cruiser(0));

        let mut rng = new_rng(seed);
        for _ in 0..5 {
            let p = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_vector(&vector![rng.gen_range(1000.0..1500.0), 0.0]);
            let v = Rotation2::new(rng.gen_range(0.0..std::f64::consts::TAU))
                .transform_vector(&vector![rng.gen_range(0.0..300.0), 0.0]);
            ship::create(sim, p, v, std::f64::consts::PI, fighter(1));
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tutorial_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![
            builtin("tutorial/tutorial11.initial"),
            builtin("tutorial/tutorial11.enemy"),
        ]
    }

    fn solution(&self) -> Code {
        builtin("tutorial/tutorial11.solution")
    }
}

struct PrimitiveDuel {
    ship0: Option<ShipHandle>,
    ship1: Option<ShipHandle>,
}

impl PrimitiveDuel {
    fn new() -> Self {
        Self {
            ship0: None,
            ship1: None,
        }
    }
}

impl Scenario for PrimitiveDuel {
    fn name(&self) -> String {
        "primitive_duel".into()
    }

    fn human_name(&self) -> String {
        "Primitive Duel".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut rng = new_rng(seed);
        let angle = rng.gen_range(0.0..TAU);
        let rot = Rotation2::new(angle);
        let distance = rng.gen_range(2000.0..4000.0);

        self.ship0 = Some(ship::create(
            sim,
            rot.transform_vector(&vector![-0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            0.0,
            fighter_without_missiles_or_radar(0),
        ));
        self.ship1 = Some(ship::create(
            sim,
            rot.transform_vector(&vector![0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            std::f64::consts::PI,
            fighter_without_missiles_or_radar(1),
        ));
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }

    fn tick(&mut self, sim: &mut Simulation) {
        if !sim.ships.contains(self.ship0.unwrap()) || !sim.ships.contains(self.ship1.unwrap()) {
            return;
        }

        let ship0_position = sim.ship(self.ship0.unwrap()).position().vector;
        let ship1_position = sim.ship(self.ship1.unwrap()).position().vector;

        if let Some(c) = sim.ship_controllers.get_mut(&self.ship0.unwrap()) {
            c.write_target(ship1_position);
        }
        if let Some(c) = sim.ship_controllers.get_mut(&self.ship1.unwrap()) {
            c.write_target(ship0_position);
        }
    }
}

struct FighterDuel {}

impl FighterDuel {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for FighterDuel {
    fn name(&self) -> String {
        "fighter_duel".into()
    }

    fn human_name(&self) -> String {
        "Fighter Duel".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut rng = new_rng(seed);
        let angle = rng.gen_range(0.0..TAU);
        let rot = Rotation2::new(angle);
        let distance = rng.gen_range(2000.0..4000.0);

        ship::create(
            sim,
            rot.transform_vector(&vector![-0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            0.0,
            fighter(0),
        );
        ship::create(
            sim,
            rot.transform_vector(&vector![0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            std::f64::consts::PI,
            fighter(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }
}

struct FrigateDuel {}

impl FrigateDuel {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for FrigateDuel {
    fn name(&self) -> String {
        "frigate_duel".into()
    }

    fn human_name(&self) -> String {
        "Frigate Duel".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut rng = new_rng(seed);
        let angle = rng.gen_range(0.0..TAU);
        let rot = Rotation2::new(angle);
        let distance = rng.gen_range(4000.0..8000.0);

        ship::create(
            sim,
            rot.transform_vector(&vector![-0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            0.0,
            frigate(0),
        );
        ship::create(
            sim,
            rot.transform_vector(&vector![0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            std::f64::consts::PI,
            frigate(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }
}

struct CruiserDuel {}

impl CruiserDuel {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for CruiserDuel {
    fn name(&self) -> String {
        "cruiser_duel".into()
    }

    fn human_name(&self) -> String {
        "Cruiser Duel".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);

        let mut rng = new_rng(seed);
        let angle = rng.gen_range(0.0..TAU);
        let rot = Rotation2::new(angle);
        let distance = rng.gen_range(5000.0..10000.0);

        ship::create(
            sim,
            rot.transform_vector(&vector![-0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            0.0,
            cruiser(0),
        );
        ship::create(
            sim,
            rot.transform_vector(&vector![0.5, 0.0]) * distance,
            vector![0.0, 0.0],
            std::f64::consts::PI,
            cruiser(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }
}

struct FrigateVsCruiser {}

impl FrigateVsCruiser {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for FrigateVsCruiser {
    fn name(&self) -> String {
        "frigate_vs_cruiser".into()
    }

    fn init(&mut self, sim: &mut Simulation, _seed: u32) {
        add_walls(sim);
        ship::create(
            sim,
            vector![-1000.0, -500.0],
            vector![0.0, 0.0],
            0.0,
            frigate(0),
        );
        ship::create(
            sim,
            vector![1000.0, 500.0],
            vector![0.0, 0.0],
            std::f64::consts::PI,
            cruiser(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![reference_ai(), reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }
}

struct CruiserVsFrigate {}

impl CruiserVsFrigate {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for CruiserVsFrigate {
    fn name(&self) -> String {
        "cruiser_vs_frigate".into()
    }

    fn init(&mut self, sim: &mut Simulation, _seed: u32) {
        add_walls(sim);
        ship::create(
            sim,
            vector![-1000.0, -500.0],
            vector![0.0, 0.0],
            0.0,
            cruiser(0),
        );
        ship::create(
            sim,
            vector![1000.0, 500.0],
            vector![0.0, 0.0],
            std::f64::consts::PI,
            frigate(1),
        );
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![reference_ai(), reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }
}

struct Furball {}

impl Furball {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Furball {
    fn name(&self) -> String {
        "furball".into()
    }

    fn human_name(&self) -> String {
        "Furball".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        let mut rng = new_rng(seed);
        for team in 0..2 {
            let fleet_radius = 500.0;
            let range = -fleet_radius..fleet_radius;
            let center = vector![(team as f64 - 0.5) * 2000.0 * 2.0, 0.0];
            let heading = if team == 0 { 0.0 } else { std::f64::consts::PI };
            for _ in 0..10 {
                let offset = vector![rng.gen_range(range.clone()), rng.gen_range(range.clone())];
                ship::create(
                    sim,
                    center + offset,
                    vector![0.0, 0.0],
                    heading,
                    fighter(team),
                );
            }
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }
}

struct Fleet {}

impl Fleet {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Fleet {
    fn name(&self) -> String {
        "fleet".into()
    }

    fn human_name(&self) -> String {
        "Fleet".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        let mut rng = new_rng(seed);
        for team in 0..2 {
            let signum = if team == 0 { -1.0 } else { 1.0 };
            let center = point![signum * 8000.0, rng.gen_range(-6000.0..6000.0)];
            let heading = if team == 0 { 0.0 } else { std::f64::consts::PI };
            let scale = 1;
            let num_fighters = scale * 40;
            let num_frigates = scale * 4;
            let num_cruisers = scale * 2;
            for i in 0..num_fighters {
                ship::create(
                    sim,
                    vector![
                        center.x - signum * 200.0,
                        center.y + i as f64 * 50.0 - (num_fighters - 1) as f64 * 25.0
                    ],
                    vector![0.0, 0.0],
                    heading,
                    fighter(team),
                );
            }
            for i in 0..num_frigates {
                ship::create(
                    sim,
                    vector![
                        center.x,
                        center.y + i as f64 * 300.0 - 150.0 * (num_frigates - 1) as f64
                    ],
                    vector![0.0, 0.0],
                    heading,
                    frigate(team),
                );
            }
            for i in 0..num_cruisers {
                ship::create(
                    sim,
                    vector![
                        center.x + signum * 500.0,
                        center.y + 400.0 * i as f64 - 200.0 * (num_cruisers - 1) as f64
                    ],
                    vector![0.0, 0.0],
                    heading,
                    cruiser(team),
                );
            }
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_capital_ship_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }
}

struct Belt {}

impl Belt {
    fn new() -> Self {
        Self {}
    }
}

impl Scenario for Belt {
    fn name(&self) -> String {
        "belt".into()
    }

    fn human_name(&self) -> String {
        "Belt".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        add_walls(sim);
        let mut rng = new_rng(seed);
        for team in 0..2 {
            let signum = if team == 0 { -1.0 } else { 1.0 };
            let center = point![rng.gen_range(-6000.0..6000.0), signum * 8000.0];
            let heading = if team == 0 { -TAU / 4.0 } else { TAU / 4.0 };
            let num_fighters = 8;
            let num_frigates = 2;
            for i in 0..(num_fighters / 2) {
                for j in [-1.0, 1.0] {
                    ship::create(
                        sim,
                        vector![center.x + j * (1000.0 + i as f64 * 100.0), center.y],
                        vector![0.0, 0.0],
                        heading,
                        fighter(team),
                    );
                }
            }
            for i in 0..(num_frigates / 2) {
                for j in [-1.0, 1.0] {
                    ship::create(
                        sim,
                        vector![center.x + j * (500.0 + i as f64 * 200.0), center.y],
                        vector![0.0, 0.0],
                        heading,
                        frigate(team),
                    );
                }
            }
            ship::create(
                sim,
                center.coords,
                vector![0.0, 0.0],
                heading,
                cruiser(team),
            );
        }

        let bound = vector![(WORLD_SIZE / 2.0) * 0.9, (WORLD_SIZE / 4.0)];
        for _ in 0..100 {
            let mut data = asteroid(rng.gen_range(0..30));
            data.health = 10000.0;
            ship::create(
                sim,
                vector![
                    rng.gen_range(-bound.x..bound.x),
                    rng.gen_range(-bound.y..bound.y)
                ],
                vector![rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)],
                rng.gen_range(0.0..(2.0 * std::f64::consts::PI)),
                data,
            );
        }
    }

    fn status(&self, sim: &Simulation) -> Status {
        check_capital_ship_tournament_victory(sim)
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }

    fn solution(&self) -> Code {
        reference_ai()
    }

    fn is_tournament(&self) -> bool {
        true
    }
}

struct FrigatePointDefense {}

impl Scenario for FrigatePointDefense {
    fn name(&self) -> String {
        "frigate_point_defense".into()
    }

    fn init(&mut self, sim: &mut Simulation, seed: u32) {
        let mut rng = new_rng(seed);

        add_walls(sim);
        let mut data = frigate(0);
        data.missile_launchers.clear();
        ship::create(sim, vector![0.0, 0.0], vector![0.0, 0.0], 0.0, data);

        for i in 1..10 {
            let distance = (i as f64) * 1000.0;
            let angle = rng.gen_range(0.0..TAU);
            let position = Rotation2::new(angle) * vector![distance, 0.0];
            let velocity = Rotation2::new(angle) * vector![0.0, rng.gen_range(-2000.0..2000.0)];
            let mut data = missile(1);
            data.ttl = None;
            ship::create(sim, position, velocity, angle + PI, data);
        }
    }

    fn status(&self, _sim: &Simulation) -> Status {
        Status::Running
    }

    fn initial_code(&self) -> Vec<Code> {
        vec![Code::None, reference_ai()]
    }
}
