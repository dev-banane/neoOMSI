use crate::ambience;
use crate::scene::World;
use crate::traffic::Traffic;
use glam::{DVec2, DVec3, Mat4, Vec3};
use hashbrown::{HashMap, HashSet};
use omsi_render::{AlphaMode, Camera, MaterialId, MeshId, Renderer, Scene};
use omsi_sim::VehicleInstance;
use omsi_sim::crowd::{self, Block, CrowdParams, Walker};
use omsi_sim::human::{Activity, HumanType, skin};
use omsi_sim::human_omsi::{AnimInput, OmsiAnim};
use omsi_sim::traffic::{LaneKind, Network};
use omsi_vehicle::PassengerCabin;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod avatar;
mod bus;
mod cabin;
mod figures;
mod lan;
mod pavement;
mod pax;
mod render;
mod stops;
mod tickets;
mod voice;

use avatar::*;
use bus::*;
use cabin::*;
use figures::*;
use lan::*;
use pavement::*;
use pax::*;
use render::*;
use stops::*;
use voice::*;

pub use avatar::AvatarCmd;
pub use lan::{MirrorPose, placed_bus_id, remote_bus_id, remote_bus_player};
pub use voice::VoiceLine;

pub(crate) static LEFT_HAND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
pub struct Eye {
    pub pos: DVec3,
    pub fwd: DVec3,
    pub cos_half: f64,
    pub fov_y: f64,
}

impl Eye {
    pub fn of(cam: &Camera, aspect: f32) -> Eye {
        let half_v = (cam.fov_deg as f64 * 0.5).to_radians();
        let half_diag = (half_v.tan() * (1.0 + (aspect as f64).powi(2)).sqrt()).atan();
        Eye {
            pos: cam.position,
            fwd: cam.forward().as_dvec3().normalize_or_zero(),
            cos_half: (half_diag + 10f64.to_radians())
                .min(89f64.to_radians())
                .cos(),
            fov_y: (cam.fov_deg as f64).to_radians().max(1e-3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusId {
    Player,
    Ai(u64),
}

#[derive(Debug, Clone)]
enum State {
    Strolling(PedWalk),
    Idle,
    Standing,
    Pax(Box<Pax>),
}

impl State {
    fn name(&self) -> &'static str {
        match self {
            State::Strolling(_) | State::Standing => "WalkStreet",
            State::Idle => "Idle",
            State::Pax(p) => p.task.name(),
        }
    }

    fn bus(&self) -> Option<BusId> {
        match self {
            State::Pax(p) => p.inside.or(p.bus),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Place {
    Ground,
    Bus(BusId, Vec3),
}

pub struct Person {
    id: u32,
    ty: Arc<HumanType>,
    variant: usize,
    meshes: Vec<(MeshId, usize)>,
    position: DVec3,
    heading: f64,
    lheading: f64,
    place: Place,
    vel: DVec2,
    pace: f64,
    activity: Activity,
    anim: OmsiAnim,
    state: State,
    skins: Vec<(Vec<Vec3>, Vec<Vec3>)>,
    skin_bones: Option<[glam::Affine3A; omsi_sim::human::SLOTS]>,
    fresh_levels: u32,
    changed_levels: u32,
    level: usize,
    interior: f32,
    lit: f32,
    tilt: Mat4,
    age: f32,
    stuck: f32,
    ghost: f32,
    car_wait: f32,
    detour: f32,
    detour_side: f64,
    why: &'static str,
    skinned: bool,
    since_posed: u32,
    avatar: bool,
    remote: bool,
    blob: usize,
    blob_shown: bool,
}

impl Person {
    pub fn state_name(&self) -> String {
        format!("#{} {} ({})", self.id, self.state.name(), self.why)
    }

    pub fn position(&self) -> DVec3 {
        self.position
    }

    fn label(&self) -> String {
        format!("#{} {}", self.id, file_name(&self.ty))
    }
}

struct Want {
    vel: DVec2,
    face: Option<f64>,
    give: f64,
    corridor: Option<(DVec2, DVec2, f64)>,
}

impl Want {
    fn stand(face: Option<f64>) -> Want {
        Want {
            vel: DVec2::ZERO,
            face,
            give: 0.35,
            corridor: None,
        }
    }
}

#[derive(Default)]
struct Gpu {
    textures: HashMap<PathBuf, Option<omsi_render::TextureId>>,
    materials: HashMap<(usize, usize, usize), Vec<MaterialId>>,
    spare: HashMap<(usize, usize, usize), Vec<(MeshId, usize)>>,
    hidden: Vec<usize>,
    blob: Option<(MeshId, MaterialId)>,
    spare_blobs: Vec<usize>,
}

pub struct Humans {
    types: Vec<Arc<HumanType>>,
    alternates: HashMap<String, Vec<Arc<HumanType>>>,
    pub people: Vec<Person>,
    rng: u64,
    next_id: u32,
    time: f64,
    wall_cells: HashMap<(i32, i32, i32), Vec<(Block, f64, f64)>>,
    wall_key: (usize, usize, usize, f64),
    cabins: HashMap<Vec<PathBuf>, Option<Arc<Cabin>>>,
    player_cabin: Option<Arc<Cabin>>,
    seats: HashMap<BusId, Vec<bool>>,
    stops: HashMap<i64, PaxStop>,
    odometer: HashMap<BusId, f64>,
    pax_req: HashMap<BusId, (Vec<bool>, Vec<bool>)>,
    desk_busy: Option<u32>,
    pardons: u8,
    ped: Option<PedNet>,
    gpu: Gpu,
    ai_visits: HashMap<u64, (i64, f64)>,
    holds: Vec<(u64, f32)>,
    ai_requests: Vec<(u64, Vec<bool>, Vec<bool>)>,
    pub tickets: Option<Arc<omsi_content::tickets::TicketPack>>,
    pub request: Option<(String, f32)>,
    pub paid: Option<(f32, f32)>,
    pub change_due: Option<f32>,
    pub money: Option<crate::money::Money>,
    pub stop_request: bool,
    pub tickets_sold: u32,
    pub ticket_cash: f32,
    pub boarded: u32,
    pub served: u32,
    pub stepped_in: u32,
    pub content: u32,
    pub ticket_requests: u32,
    pub ticket_points: u32,
    pub entry_req: Vec<bool>,
    pub exit_req: Vec<bool>,
    sync_frame: u32,
    footfalls: Vec<ambience::Footfall>,
    pub density: f32,
    pub time_of_day: f64,
    pub delay: f64,
    root: PathBuf,
    voice_lines: Vec<VoiceLine>,
    voice_said: HashMap<PathBuf, f64>,
    /// 0 everything, 1 only the ticket asked for, 2 nothing.
    pub voices: u8,
    last_chat: f64,
    avatars: Avatars,
    last_buses: Vec<BusNow>,
    pub avatar_only: bool,
    pub stop_targets: Option<HashMap<i64, Vec<(String, HashSet<String>)>>>,
    pub stop_names: Option<HashMap<i64, String>>,
    stamped: Vec<BusId>,
    pub pedestrians: usize,
    stroll_timer: f32,
    pub exact_fare: bool,
    pub boarding: String,
    /// Free seats before standing places; off, OMSI's random choice among all places.
    pub prefer_seats: bool,
    pub give_ticket: bool,
    pub ticket_key: String,
    pub eye: Option<Eye>,
    center: DVec3,
    message: Option<String>,
    tick_stats: (u32, f64, f64),
    map_humans_done: bool,
    last_sync: f64,
    pose_stats: (u32, usize, f64, f64),
    tiles_seen: u64,
    lan: Lan,
    pub lan_centers: Vec<DVec3>,
    placed_now: Vec<BusNow>,
    comfort: RideComfort,
    pub natural: bool,
}

impl Humans {
    pub fn new(root: &Path) -> Humans {
        let (types, alternates) = load_figures(root);
        Humans {
            types,
            alternates,
            people: Vec::new(),
            rng: 0x1234_5678_9ABC_DEF1,
            next_id: 1,
            time: 0.0,
            wall_cells: HashMap::new(),
            wall_key: (0, 0, 0, 0.0),
            cabins: HashMap::new(),
            player_cabin: None,
            seats: HashMap::new(),
            stops: HashMap::new(),
            odometer: HashMap::new(),
            pax_req: HashMap::new(),
            desk_busy: None,
            pardons: 0,
            ped: None,
            gpu: Gpu::default(),
            ai_visits: HashMap::new(),
            holds: Vec::new(),
            ai_requests: Vec::new(),
            tickets: None,
            request: None,
            paid: None,
            change_due: None,
            money: None,
            stop_request: false,
            tickets_sold: 0,
            ticket_cash: 0.0,
            boarded: 0,
            served: 0,
            stepped_in: 0,
            content: 0,
            ticket_requests: 0,
            ticket_points: 0,
            entry_req: Vec::new(),
            exit_req: Vec::new(),
            sync_frame: 0,
            footfalls: Vec::new(),
            density: 1.0,
            time_of_day: 12.0 * 3600.0,
            delay: 0.0,
            root: root.to_path_buf(),
            voice_lines: Vec::new(),
            voice_said: HashMap::new(),
            voices: 0,
            last_chat: -1e9,
            avatars: Avatars::default(),
            last_buses: Vec::new(),
            avatar_only: false,
            stop_targets: None,
            stop_names: None,
            stamped: Vec::new(),
            pedestrians: 14,
            stroll_timer: 0.0,
            exact_fare: true,
            boarding: "auto".into(),
            prefer_seats: false,
            give_ticket: false,
            ticket_key: "T".into(),
            eye: None,
            center: DVec3::ZERO,
            message: None,
            tick_stats: (0, 0.0, 0.0),
            map_humans_done: false,
            last_sync: 0.0,
            pose_stats: (0, 0, 0.0, 0.0),
            tiles_seen: 0,
            lan: Lan::default(),
            lan_centers: Vec::new(),
            placed_now: Vec::new(),
            comfort: RideComfort::default(),
            natural: false,
        }
    }

    fn rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn rand_f(&mut self) -> f64 {
        (self.rand() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn walk_pace(&mut self) -> f64 {
        1.1 + (self.rand_f() * 2.0 - 1.0) * 0.2
    }

    fn ride_km(&mut self) -> f32 {
        self.rand_f() as f32 * 19.0 + 1.0
    }

    fn seen(&self, p: DVec3) -> bool {
        match self.eye {
            None => (p - self.center).length() < 150.0,
            Some(e) => {
                let d = p + DVec3::Z * 0.9 - e.pos;
                let dist = d.length();
                dist <= 230.0 && (dist < 3.0 || d.dot(e.fwd) / dist > e.cos_half)
            }
        }
    }

    pub fn riding(&self) -> usize {
        self.people
            .iter()
            .filter(|p| matches!(p.place, Place::Bus(BusId::Player, _)))
            .count()
    }

    pub fn on_foot(&self) -> Vec<(DVec2, DVec2, bool)> {
        self.people
            .iter()
            .filter(|p| p.place == Place::Ground)
            .map(|p| {
                let waiting = matches!(&p.state, State::Pax(x) if x.inside.is_none());
                (p.position.truncate(), p.vel, waiting)
            })
            .collect()
    }

    pub fn strollers(&self) -> Vec<(usize, f32)> {
        self.people
            .iter()
            .filter_map(|p| match &p.state {
                State::Strolling(walk) => walk
                    .legs
                    .get(walk.leg)
                    .map(|leg| (leg.lane, leg.dist(walk.s))),
                _ => None,
            })
            .collect()
    }

    fn spawn(
        &mut self,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        position: DVec3,
        heading: f64,
        state: State,
    ) -> Option<usize> {
        self.spawn_as(world, renderer, scene, position, heading, state, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_as(
        &mut self,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        mut position: DVec3,
        heading: f64,
        mut state: State,
        kind: Option<usize>,
    ) -> Option<usize> {
        self.use_map_humans(world);
        if self.types.is_empty() {
            return None;
        }
        // on the pavement they will walk on, not on the terrain under it
        if let Some(z) = world.walk_height_near(position.x, position.y, position.z) {
            if (z - position.z).abs() < 3.0 {
                position.z = z;
            }
        }
        let (ty, variant) = self.pick_figure(position, kind);
        let meshes = self.add_meshes(world, renderer, scene, &ty, variant, position);
        let blob = self.add_blob(renderer, scene, position);
        let mut pace = self.walk_pace();
        if self.natural {
            pace = natural_pace(&ty.def, pace);
            if let State::Pax(x) = &mut state {
                x.walk_speed = pace as f32;
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        log::debug!(
            "#{id} {} appears at {position:.1}: {}",
            file_name(&ty),
            state.name()
        );
        self.people.push(Person {
            id,
            age: ty.def.age.map(|a| a as f32).unwrap_or(40.0),
            ty,
            variant,
            meshes,
            position,
            heading,
            lheading: 0.0,
            place: Place::Ground,
            vel: DVec2::ZERO,
            pace,
            activity: Activity::Stand,
            anim: OmsiAnim::default(),
            state,
            skins: Vec::new(),
            skin_bones: None,
            fresh_levels: 0,
            changed_levels: 0,
            level: 0,
            interior: 0.0,
            lit: 0.0,
            tilt: Mat4::IDENTITY,
            stuck: 0.0,
            ghost: 0.0,
            car_wait: 0.0,
            detour: 0.0,
            detour_side: 0.0,
            why: "",
            skinned: false,
            since_posed: 0,
            avatar: false,
            remote: false,
            blob,
            blob_shown: false,
        });
        Some(self.people.len() - 1)
    }

    fn free_seat(&mut self, bus: BusId, seat: usize) {
        if let Some(t) = self.seats.get_mut(&bus).and_then(|v| v.get_mut(seat)) {
            *t = false;
        }
    }

    fn release(&mut self, i: usize) {
        let id = self.people[i].id;
        let State::Pax(x) = &mut self.people[i].state else {
            return;
        };
        let (stop, spot, bus, seat) = (x.stop, x.spot.take(), x.bus.or(x.inside), x.seat.take());
        if let (Some(s), Some(k)) = (stop, spot) {
            self.free_spot(s, k);
        }
        if let (Some(b), Some(k)) = (bus, seat) {
            self.free_seat(b, k);
        }
        if self.desk_busy == Some(id) {
            self.desk_busy = None;
            self.request = None;
        }
    }

    fn remove_person(&mut self, i: usize) {
        self.release(i);
        let p = self.people.swap_remove(i);
        log::debug!("{} taken away ({})", p.label(), p.state.name());
        self.retire(&p);
    }

    pub fn run_over(&mut self, bus: &VehicleInstance) -> u32 {
        if bus.physics.velocity_kmh().abs() < 5.0 {
            return 0;
        }
        let Some(bb) = bus.ty.def.bounding_box else {
            return 0;
        };
        let (half_x, half_y) = ((bb[0] - bb[3]).abs() / 2.0, (bb[1] - bb[4]).abs() / 2.0);
        let inv = bus.body_rotation().transpose();
        let mut knocked = Vec::new();
        for (i, p) in self.people.iter().enumerate() {
            let counts = match &p.state {
                State::Pax(x) => x.task == Task::WaitingForBus,
                State::Strolling(_) | State::Standing => true,
                State::Idle => false,
            };
            if p.place != Place::Ground || !counts {
                continue;
            }
            let local = inv.transform_vector3((p.position - bus.position).as_vec3());
            if local.x.abs() < half_x + 0.2 && local.y.abs() < half_y + 0.2 && local.z.abs() < 3.0 {
                knocked.push(i);
            }
        }
        for &i in knocked.iter().rev() {
            if let State::Pax(x) = &self.people[i].state {
                let (at, h, stop) = (x.pos, x.yaw.to_degrees(), x.stop);
                self.release(i);
                self.walk_street(i, at, h, stop);
            }
        }
        knocked.len() as u32
    }

    pub fn take_message(&mut self) -> Option<String> {
        self.message.take()
    }

    pub fn positions(&self) -> Vec<(String, DVec3)> {
        self.people
            .iter()
            .map(|p| (p.state.name().to_string(), p.position))
            .collect()
    }

    pub fn summary(&self) -> String {
        let mut counts: std::collections::BTreeMap<&'static str, usize> = Default::default();
        for p in &self.people {
            *counts.entry(p.state.name()).or_default() += 1;
        }
        let mut out = counts
            .iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect::<Vec<_>>()
            .join(", ");
        let (n, total, worst) = self.tick_stats;
        if n > 0 {
            out.push_str(&format!(
                "; {:.2} ms a frame, longest {worst:.1} ms",
                total / n as f64
            ));
        }
        let (frames, posed, ms, up) = self.pose_stats;
        if frames > 0 {
            let f = frames as f64;
            out.push_str(&format!(
                "; posing {:.2} ms a frame ({:.1} people, {:.2} ms of it uploading and placing)",
                ms / f,
                posed as f64 / f,
                up / f
            ));
        }
        out
    }

    /// Returns whether a passenger took the printed ticket (the caller resets `GivenTicket`).
    pub fn tick(
        &mut self,
        dt: f32,
        world: &World,
        bus: Option<&VehicleInstance>,
        traffic: Option<&Traffic>,
        renderer: &Renderer,
        scene: &mut Scene,
    ) -> bool {
        let started = std::time::Instant::now();
        let took = self.step(dt, world, bus, traffic, renderer, scene);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        self.tick_stats.0 += 1;
        self.tick_stats.1 += ms;
        self.tick_stats.2 = self.tick_stats.2.max(ms);
        took
    }

    fn step(
        &mut self,
        dt: f32,
        world: &World,
        bus: Option<&VehicleInstance>,
        traffic: Option<&Traffic>,
        renderer: &Renderer,
        scene: &mut Scene,
    ) -> bool {
        self.use_map_humans(world);
        self.time += dt as f64;
        let net = traffic.map(|t| &t.net);
        if let Some(b) = bus {
            self.center = b.position;
        } else if let Some(e) = self.eye {
            self.center = e.pos;
        }
        let generation = world
            .tiles_generation
            .load(std::sync::atomic::Ordering::Relaxed);
        if generation != self.tiles_seen {
            self.tiles_seen = generation;
            self.tiles_changed(world);
        }
        if let Some(n) = net {
            self.update_pavements(n);
            self.stroll_timer -= dt;
            if self.stroll_timer <= 0.0 {
                self.stroll_timer = 1.0;
                self.populate_with(world, Some(n), self.center);
                if !self.lan.mirror {
                    self.populate_on_foot(world, n, renderer, scene);
                    self.populate_lan_centers(world, n, renderer, scene);
                }
            }
        }
        let buses = self.gather_buses(world, bus, traffic);
        self.last_buses = buses.clone();
        let at_stops = self.register_buses(&buses, dt);
        let f = Frame::new(world, bus, buses, at_stops);
        self.claim_waiting();
        self.ride_comfort(dt, &f);
        if !self.avatar_only {
            self.stops_tick(dt, world, renderer, scene);
        }
        let mut taken_ticket = false;
        let mut remove: Vec<usize> = Vec::new();
        self.pax_frame(dt, &f, renderer, scene, &mut taken_ticket, &mut remove);
        self.walk_pedestrians(dt, &f, net, traffic, &mut remove);
        self.pax_room(dt, &f);
        self.animate(dt, &f);
        remove.sort_unstable();
        remove.dedup();
        for i in remove.into_iter().rev() {
            self.remove_person(i);
        }
        self.give_ticket = false;
        taken_ticket
    }
}

fn wrap_heading(h: f64) -> f64 {
    h.rem_euclid(360.0)
}

/// 0..1, the same for a person on every machine of a LAN game.
fn person_hash(id: u32, n: u32) -> f64 {
    let mut x = (id as u64) << 32 | n as u64;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Walking speed by age and height, `omsi` (Omsi.exe's 0.9..1.3 m/s draw) setting where in
/// the range a person is: about 1.35 m/s for an adult, 1.0 at 75, 1.1 for a young child.
fn natural_pace(def: &omsi_content::Human, omsi: f64) -> f64 {
    let age = def.age.map_or(40.0, |a| a as f64);
    let by_age = match age {
        a if a < 8.0 => 1.0,
        a if a < 13.0 => 1.0 + (a - 8.0) * 0.06,
        a if a < 60.0 => 1.35,
        a => (1.35 - (a - 60.0) * 0.022).max(0.8),
    };
    let tall = if age >= 13.0 && def.height > 1.0 {
        (def.height as f64 / 1.75).sqrt().clamp(0.9, 1.07)
    } else {
        1.0
    };
    by_age * tall * (1.0 + 0.5 * (omsi - 1.1))
}
