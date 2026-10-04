//! Passengers as Omsi.exe runs them (sub_62a6a0). They do not avoid each other: somebody
//! within 0.6 m in front stops them (sub_626860).

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Task {
    Nothing,
    WaitingForBus,
    ToBus,
    WalkingToBus,
    InBusToPlace,
    InBusToExit,
    WalkingToBusstop,
    SittingInBus,
}

impl Task {
    pub(super) fn name(self) -> &'static str {
        match self {
            Task::Nothing => "DoNothing",
            Task::WaitingForBus => "WaitingForBus",
            Task::ToBus => "BusComing",
            Task::WalkingToBus => "WalkingToBus",
            Task::InBusToPlace => "WalkingInBusToPlace",
            Task::InBusToExit => "WalkingInBusToExit",
            Task::WalkingToBusstop => "WalkingToBusstop",
            Task::SittingInBus => "SittingInBus",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Move {
    Stand,
    ToTarget,
    ShortOfTarget,
    AtTarget,
    Path,
    ShortOfPathEnd,
    PathEnd,
    Turn,
}

impl Move {
    pub(super) fn walking(self) -> bool {
        matches!(self, Move::ToTarget | Move::Path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ticket {
    None,
    Stamp,
    Buy,
}

pub(super) const OUTSIDE_ROOM: f32 = 50.0;

/// `pos` and `yaw` are in the bus frame while `inside` one; `yaw` is Direct3D's (radians,
/// clockwise from forward).
#[derive(Debug, Clone)]
pub(super) struct Pax {
    pub(super) task: Task,
    pub(super) st: Move,
    pub(super) inside: Option<BusId>,
    pub(super) pos: DVec3,
    pub(super) yaw: f64,
    pub(super) bus: Option<BusId>,
    pub(super) stop: Option<i64>,
    pub(super) spot: Option<usize>,
    pub(super) target: DVec3,
    pub(super) target_bus: bool,
    pub(super) target_yaw: f64,
    pub(super) pt: Option<usize>,
    pub(super) pt_target: Option<usize>,
    pub(super) short: bool,
    pub(super) clamp: bool,
    pub(super) clamp_open: bool,
    pub(super) clamp_left: bool,
    pub(super) clamp_x: f64,
    pub(super) dest: Option<String>,
    pub(super) line: Option<usize>,
    pub(super) ride_km: f32,
    pub(super) km_start: f64,
    pub(super) seat: Option<usize>,
    pub(super) ticket: Ticket,
    pub(super) ticket_id: u8,
    pub(super) price: f32,
    pub(super) paid: f32,
    pub(super) sub: u8,
    pub(super) door: Option<usize>,
    pub(super) seat_h: f32,
    pub(super) pax_state: u8,
    pub(super) timer: f32,
    pub(super) smooth: bool,
    pub(super) reach: bool,
    pub(super) look_driver: bool,
    pub(super) reach_at: Vec3,
    pub(super) room: f32,
    pub(super) step_pack: Option<usize>,
    pub(super) link: Option<usize>,
    pub(super) speed_des: f32,
    pub(super) speed: f32,
    pub(super) walk_speed: f32,
    pub(super) block: u8,
    pub(super) free_r: bool,
    pub(super) jam: f32,
    pub(super) squeeze: f32,
    pub(super) discomfort: f32,
    pub(super) complaint: u8,
    pub(super) bad_at: [f32; 3],
    pub(super) moved: f32,
}

impl Pax {
    pub(super) fn new(walk_speed: f32) -> Pax {
        Pax {
            task: Task::Nothing,
            st: Move::Stand,
            inside: None,
            pos: DVec3::ZERO,
            yaw: 0.0,
            bus: None,
            stop: None,
            spot: None,
            target: DVec3::ZERO,
            target_bus: false,
            target_yaw: 0.0,
            pt: None,
            pt_target: None,
            short: false,
            clamp: false,
            clamp_open: false,
            clamp_left: false,
            clamp_x: -1e9,
            dest: None,
            line: None,
            ride_km: 0.0,
            km_start: 0.0,
            seat: None,
            ticket: Ticket::None,
            ticket_id: 0,
            price: 0.0,
            paid: 0.0,
            sub: 0,
            door: None,
            seat_h: 0.0,
            pax_state: 0,
            timer: 0.0,
            smooth: false,
            reach: false,
            look_driver: false,
            reach_at: Vec3::ZERO,
            room: OUTSIDE_ROOM,
            step_pack: None,
            link: None,
            speed_des: 0.0,
            speed: 0.0,
            walk_speed,
            block: 0,
            free_r: true,
            jam: 0.0,
            squeeze: 0.0,
            discomfort: 0.0,
            complaint: 0,
            bad_at: [0.0; 3],
            moved: 0.0,
        }
    }

    pub(super) fn take_seat(&mut self, s: &Seat, seat_height: f32) {
        if s.seated {
            self.seat_h = s.height;
            self.pos = (s.pos - Vec3::Z * seat_height).as_dvec3();
            self.pax_state = 2;
        } else {
            self.pos = s.pos.as_dvec3();
            self.pax_state = 0;
        }
        self.yaw = (s.rot as f64).to_radians();
    }

    fn walking_in(&mut self) {
        self.st = Move::Path;
        self.pax_state = 1;
    }
}

fn wrap(mut a: f64) -> f64 {
    use std::f64::consts::PI;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

fn yaw_of(d: DVec2) -> f64 {
    d.x.atan2(d.y)
}

impl Humans {
    pub fn seed_riders(
        &mut self,
        n: usize,
        bus: &VehicleInstance,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
    ) {
        let Some(cabin) = self.cabin_for(bus) else {
            return;
        };
        let trailers = part_frames(bus, &cabin);
        let rot = bus.body_rotation();
        for _ in 0..n {
            let Some(k) = self.reserve_place(BusId::Player, cabin.seats.len()) else {
                break;
            };
            let mut pax = Pax::new(self.walk_pace() as f32);
            pax.bus = Some(BusId::Player);
            pax.inside = Some(BusId::Player);
            pax.seat = Some(k);
            pax.ride_km = self.ride_km();
            pax.task = Task::SittingInBus;
            let s = &cabin.seats[k];
            let at = train_point(bus.position, &rot, &trailers, s.pos);
            let Some(i) = self.spawn(
                world,
                renderer,
                scene,
                at,
                bus.heading,
                State::Pax(Box::new(pax)),
            ) else {
                self.free_seat(BusId::Player, k);
                break;
            };
            let seat_height = self.people[i].ty.def.seat_height;
            self.pax_mut(i).unwrap().take_seat(s, seat_height);
            self.people[i].place = Place::Bus(BusId::Player, s.pos);
        }
    }

    pub(super) fn pax(&self, i: usize) -> Option<&Pax> {
        match &self.people[i].state {
            State::Pax(p) => Some(p),
            _ => None,
        }
    }

    pub(super) fn pax_mut(&mut self, i: usize) -> Option<&mut Pax> {
        match &mut self.people[i].state {
            State::Pax(p) => Some(p),
            _ => None,
        }
    }

    /// A free place at random; none free, nobody gets on.
    pub(super) fn reserve_place(&mut self, bus: BusId, n: usize) -> Option<usize> {
        let seats = self.seats.entry(bus).or_insert_with(|| vec![false; n]);
        if seats.len() < n {
            seats.resize(n, false);
        }
        let free: Vec<usize> = (0..n).filter(|k| !seats[*k]).collect();
        if free.is_empty() {
            return None;
        }
        let k = free[(self.rand() as usize) % free.len()];
        self.seats.get_mut(&bus).unwrap()[k] = true;
        Some(k)
    }

    fn request_door(&mut self, bus: BusId, door: usize, exit: bool) {
        if let Some((e, x)) = self.pax_req.get_mut(&bus) {
            if let Some(r) = if exit { x } else { e }.get_mut(door) {
                *r = true;
            }
        }
    }

    fn pax_world(&self, p: &Pax, f: &Frame) -> Option<(DVec3, f64)> {
        match p.inside {
            None => Some((p.pos, p.yaw.to_degrees())),
            Some(b) => {
                let bn = f.bus(Some(b))?;
                let l = p.pos.as_vec3();
                Some((bn.world(l), bn.heading_at(l) + p.yaw.to_degrees()))
            }
        }
    }

    pub(super) fn pax_frame(
        &mut self,
        dt: f32,
        f: &Frame,
        renderer: &Renderer,
        scene: &mut Scene,
        taken_ticket: &mut bool,
        remove: &mut Vec<usize>,
    ) {
        self.entry_req
            .iter_mut()
            .chain(self.exit_req.iter_mut())
            .for_each(|r| *r = false);
        self.pax_req = f
            .buses()
            .iter()
            .map(|bn| {
                (
                    bn.id,
                    (
                        vec![false; bn.cabin.entries.len()],
                        vec![false; bn.cabin.exits.len()],
                    ),
                )
            })
            .collect();
        for i in 0..self.people.len() {
            if self.pax(i).is_some() && !remove.contains(&i) {
                self.pax_tick(i, dt, f, renderer, scene, taken_ticket, remove);
            }
        }
        if let Some((e, x)) = self.pax_req.get(&BusId::Player) {
            self.entry_req = e.clone();
            self.exit_req = x.clone();
        }
        self.ai_requests.clear();
        for (b, (e, x)) in &self.pax_req {
            if let BusId::Ai(id) = b {
                self.ai_requests.push((*id, e.clone(), x.clone()));
            }
        }
        // timetable buses wait while people get on or off; for somebody on the way to the
        // gather point only while the bus stands in the stop's box, or it waits for good
        for bn in f.buses() {
            let BusId::Ai(id) = bn.id else { continue };
            if bn.speed.abs() > 0.5 {
                continue;
            }
            let busy = self.people.iter().any(|p| match &p.state {
                State::Pax(x) => {
                    let coming = match x.task {
                        Task::WalkingToBus => true,
                        Task::ToBus => x.stop.is_some_and(|s| self.in_stop_box(s, bn.id)),
                        _ => false,
                    };
                    x.bus == Some(bn.id)
                        && (coming || (x.task == Task::InBusToExit && x.inside == Some(bn.id)))
                }
                _ => false,
            });
            if busy {
                self.holds.push((id, 2.5));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn pax_tick(
        &mut self,
        i: usize,
        dt: f32,
        f: &Frame,
        renderer: &Renderer,
        scene: &mut Scene,
        taken_ticket: &mut bool,
        remove: &mut Vec<usize>,
    ) {
        let p = self.pax_mut(i).unwrap();
        if p.bus.is_some_and(|b| !f.bus(Some(b)).is_some()) {
            p.bus = None;
        }
        if p.inside.is_some_and(|b| !f.bus(Some(b)).is_some()) {
            remove.push(i);
            return;
        }
        if p.timer > 0.0 {
            p.timer -= dt;
        }
        // the toll of a bad ride eases off by 0.2 a kilometre
        p.discomfort = match f.bus(p.inside) {
            Some(bn) => (p.discomfort - bn.speed.abs() as f32 * dt / 5000.0).max(0.0),
            None => 0.0,
        };
        self.pax_move(i, dt, f);
        self.pax_task(i, f, renderer, scene, taken_ticket);
        let Some(p) = self.pax(i).cloned() else {
            return;
        };
        // the stop they boarded at is forgotten once the bus has left it
        if matches!(
            p.task,
            Task::InBusToPlace | Task::InBusToExit | Task::SittingInBus
        ) {
            if let (Some(stop), Some(b)) = (p.stop, p.bus) {
                if !f.at_stops.get(&b).is_some_and(|r| r.near.contains(&stop)) {
                    self.pax_mut(i).unwrap().stop = None;
                }
            }
        }
        let Some((w, h)) = self.pax_world(&p, f) else {
            return;
        };
        let person = &mut self.people[i];
        person.position = w;
        person.heading = h;
        match p.inside {
            Some(b) => {
                person.place = Place::Bus(b, p.pos.as_vec3());
                person.lheading = p.yaw.to_degrees();
                if let Some(bn) = f.bus(Some(b)) {
                    person.tilt = bn.tilt_at(p.pos.as_vec3());
                    person.interior = bn.interior;
                }
            }
            None => {
                person.place = Place::Ground;
                person.interior = 0.0;
            }
        }
        person.vel = if p.st.walking() {
            DVec2::new(p.yaw.sin(), p.yaw.cos()) * p.speed as f64
        } else {
            DVec2::ZERO
        };
    }

    fn pax_move(&mut self, i: usize, dt: f32, f: &Frame) {
        let dt_ms = dt * 1000.0;
        let p0 = self.pax(i).unwrap().clone();
        let bn_in = f.bus(p0.inside);
        let bn_t = f.bus(p0.bus);
        let mut target = p0.target;
        let mut target_bus = p0.target_bus;
        // the path point walked to is the target, and stays it while waiting short of it:
        // with the target of before kept (a point of another frame) the people waiting at a
        // shut exit were lifted 40 m up (#709)
        let mut walked_to: Option<DVec3> = None;
        if p0.st == Move::Path {
            if let Some(q) = p0
                .pt
                .zip(bn_in)
                .and_then(|(pt, bn)| bn.cabin.points.get(pt))
            {
                target = q.as_dvec3();
                target_bus = true;
                walked_to = Some(target);
            }
        }
        if let (true, true, Some(bn)) = (p0.clamp, target_bus, bn_t) {
            let level = p0.clamp_open
                && p0.inside.is_none()
                && (bn.to_local(p0.pos).y as f64 - target.y).abs() <= 1.0;
            if !level {
                target.x = if p0.clamp_left {
                    target.x.min(p0.clamp_x)
                } else {
                    target.x.max(p0.clamp_x)
                };
            }
        }
        let tgt = match (target_bus, p0.inside) {
            (true, None) => bn_t.map_or(target, |bn| bn.world(target.as_vec3())),
            (false, Some(_)) => bn_in.map_or(target, |bn| bn.to_local(target).as_dvec3()),
            _ => target,
        };
        let mut d = tgt - p0.pos;
        let (mut room, mut step_pack) = (p0.room, p0.step_pack);
        if p0.inside.is_none() {
            d.z = 0.0;
            step_pack = None;
            room = OUTSIDE_ROOM;
        }
        let dist = d.length() as f32;
        let (mut st, mut pt, mut link) = (p0.st, p0.pt, p0.link);
        match st {
            Move::Path if p0.pt == p0.pt_target && dist <= 0.7 && p0.short => {
                st = Move::ShortOfPathEnd
            }
            Move::Path if dist <= 0.1 => {
                let next = match (p0.pt, p0.pt_target, bn_in) {
                    (Some(a), Some(b), Some(bn)) => bn.cabin.route_next(a, b).map(|n| (n, bn)),
                    _ => None,
                };
                match next {
                    Some(((n, l), bn)) => {
                        pt = Some(n);
                        link = Some(l);
                        step_pack = bn.cabin.link_pack.get(l).copied().flatten();
                        room = bn.cabin.link_room.get(l).copied().unwrap_or(2.0);
                    }
                    None => st = Move::PathEnd,
                }
            }
            Move::ToTarget if dist <= 0.7 && p0.short => st = Move::ShortOfTarget,
            Move::ToTarget if dist <= 0.1 => st = Move::AtTarget,
            Move::ShortOfPathEnd if !p0.short => st = Move::Path,
            Move::ShortOfTarget if !p0.short => st = Move::ToTarget,
            _ => {}
        }
        let (mut block, free_r) = if st.walking() {
            self.pax_blockers(i, f)
        } else {
            (0, true)
        };
        let p = self.pax_mut(i).unwrap();
        // people going opposite ways along an aisle or the stairs stood face to face for good:
        // held up for two seconds, they squeeze past for a second and a half
        if p.inside.is_some() {
            if p.squeeze > 0.0 {
                p.squeeze -= dt;
                block = 0;
            } else if block == 2 {
                p.jam += dt;
                if p.jam > 2.0 {
                    p.jam = 0.0;
                    p.squeeze = 1.5;
                    block = 0;
                }
            } else {
                p.jam = 0.0;
            }
        }
        if let Some(t) = walked_to {
            p.target = t;
            p.target_bus = true;
        }
        p.st = st;
        p.pt = pt;
        p.link = link;
        p.room = room;
        p.step_pack = step_pack;
        p.clamp = false;
        p.clamp_open = false;
        p.clamp_left = false;
        p.clamp_x = -1e9;
        p.moved = 0.0;
        p.speed_des = 0.0;
        p.block = block;
        p.free_r = free_r;
        let mut slope = f64::INFINITY;
        let head_des = match st {
            Move::ToTarget | Move::Path => {
                if p.inside.is_some() || p.pax_state == 2 {
                    let h = d.truncate().length();
                    slope = if h > 0.0 { d.z / h } else { f64::INFINITY };
                }
                p.speed_des = if block < 2 { p.walk_speed } else { 0.0 };
                yaw_of(d.truncate())
            }
            Move::AtTarget | Move::PathEnd => p.target_yaw,
            Move::Turn => yaw_of(d.truncate()),
            Move::Stand => {
                // on the ground, not at the height of the waiting place's object
                if p.inside.is_none() && p.pax_state != 2 {
                    if let Some(g) = f.world.walk_height_near(p.pos.x, p.pos.y, p.pos.z) {
                        p.pos.z = g;
                    }
                }
                return;
            }
            _ => p.yaw,
        };
        let mut dh = wrap(head_des - p.yaw);
        if st == Move::ToTarget && p.free_r && p.block == 2 {
            dh = -1.745;
        }
        if st != Move::Turn {
            if dh.abs() > 1.0 {
                p.speed = 0.0;
            }
            let diff = p.speed_des - p.speed;
            p.speed += diff.signum() * diff.abs().min(5.0 * dt_ms / 1000.0);
        }
        p.yaw = wrap(p.yaw + dh.signum() * dh.abs().min(dt_ms as f64 / 150.0));
        if st == Move::Turn {
            return;
        }
        let mut moved = p.speed * dt_ms / 1000.0;
        let mut step = DVec3::new(d.x, d.y, 0.0);
        let len = step.length() as f32;
        if len <= moved {
            moved = len;
        } else if len > 0.0 {
            step *= (moved / len) as f64;
        }
        p.moved = moved;
        let on_ground = p.inside.is_none() && p.pax_state != 2;
        if !on_ground {
            step.z = if slope.is_finite() {
                moved as f64 * slope
            } else {
                d.z
            };
        } else {
            let at = p.pos + step;
            step.z = f
                .world
                .walk_height_near(at.x, at.y, p.pos.z)
                .map_or(0.0, |g| g - p.pos.z);
        }
        p.pos += step;
        if let Some(floor) = p
            .stop
            .filter(|_| on_ground)
            .and_then(|s| self.stops.get(&s))
            .map(|s| s.pos.z)
        {
            let p = self.pax_mut(i).unwrap();
            p.pos.z = p.pos.z.max(floor);
        }
    }

    fn pax_blockers(&self, i: usize, f: &Frame) -> (u8, bool) {
        let me = self.pax(i).unwrap();
        let Some((my_pos, my_head)) = self.pax_world(me, f) else {
            return (0, true);
        };
        let dir = |deg: f64| DVec2::new(deg.to_radians().sin(), deg.to_radians().cos());
        let fs = dir(my_head);
        let (mut block, mut free_r) = (0u8, true);
        for (j, o) in self.people.iter().enumerate() {
            if j == i || o.avatar {
                continue;
            }
            let (o_task, o_still, o_bus, o_stop, o_sub, o_block) = match &o.state {
                State::Pax(x) => (
                    Some(x.task),
                    matches!(x.st, Move::Stand | Move::AtTarget),
                    x.bus,
                    x.stop,
                    x.sub,
                    x.block,
                ),
                State::Strolling(_) => (None, false, None, None, 0, 0),
                _ => continue,
            };
            if o_still || (o_task == Some(Task::ToBus) && me.task == Task::WalkingToBus) {
                continue;
            }
            if !(o_bus == me.bus || (o_stop.is_some() && o_stop == me.stop)) {
                continue;
            }
            let d = o.position - my_pos;
            let dist = d.truncate().length();
            if d.z >= 2.0 || !(dist < 0.6) {
                continue;
            }
            let dn = if dist > 0.0 {
                d.truncate() / dist
            } else {
                DVec2::ZERO
            };
            let fo = dir(o.heading);
            if fs.dot(dn) < 0.0 {
                block = block.max(1);
                continue;
            }
            let facing = fo.dot(fs) < 0.2 || dn.dot(fo) <= 0.0;
            if facing && o_sub == 0 {
                if o_block == 0 {
                    block = block.max(2);
                    // (D3DXVec3Cross(fs, d).y in the left-handed frame)
                    if fs.y * dn.x - fs.x * dn.y < 0.0 {
                        free_r = false;
                    }
                }
                continue;
            }
            block = block.max(3);
        }
        (block, free_r)
    }

    pub(super) fn set_task(&mut self, i: usize, t: Task, f: &Frame) {
        let Some(p) = self.pax(i).filter(|p| p.task != t) else {
            return;
        };
        log::debug!(
            "{} {} -> {}",
            self.people[i].label(),
            p.task.name(),
            t.name()
        );
        let seat_height = self.people[i].ty.def.seat_height;
        self.pax_mut(i).unwrap().task = t;
        match t {
            Task::WaitingForBus => {
                let p = self.pax(i).unwrap();
                let sp = p
                    .stop
                    .zip(p.spot)
                    .and_then(|(s, k)| self.stops.get(&s)?.spots.get(k).cloned());
                let p = self.pax_mut(i).unwrap();
                p.st = Move::Stand;
                p.pax_state = 0;
                if let Some(sp) = sp {
                    p.yaw = sp.face.to_radians();
                    p.pos = sp.pos;
                    if sp.height != 0.0 {
                        p.seat_h = sp.height;
                        p.pos = sp.pos - DVec3::Z * seat_height as f64;
                        p.pax_state = 2;
                    }
                }
            }
            Task::ToBus | Task::WalkingToBus => {
                let p = self.pax(i).unwrap();
                let stop = p.stop;
                if let (Some(s), Some(k)) = (stop, p.spot) {
                    self.free_spot(s, k);
                }
                self.pax_mut(i).unwrap().spot = None;
                if t == Task::ToBus {
                    let gather = stop.and_then(|s| self.stops.get(&s)).map(|s| s.gather);
                    let p = self.pax_mut(i).unwrap();
                    p.target = gather.unwrap_or(p.target);
                    p.target_bus = false;
                } else {
                    self.choose_entry(i, f);
                    self.pax_mut(i).unwrap().target_bus = true;
                }
                let p = self.pax_mut(i).unwrap();
                p.st = Move::ToTarget;
                p.pax_state = 1;
            }
            Task::InBusToPlace => {
                let Some(bn) = f.bus(self.pax(i).unwrap().bus) else {
                    return;
                };
                let km = self.odometer.get(&bn.id).copied().unwrap_or(0.0);
                let p = self.pax_mut(i).unwrap();
                p.pax_state = 1;
                p.km_start = km;
                p.door = None;
                let local = bn.to_local(p.pos);
                p.yaw = wrap(p.yaw - bn.heading_at(local).to_radians());
                p.pos = local.as_dvec3();
                p.inside = Some(bn.id);
                p.target_bus = true;
                p.pt =
                    bn.cabin
                        .omsi_nearest(local, &bn.cabin.all_points(), false, false, None, None);
                p.st = Move::Path;
                // in any bus but the player's they are at their place at once (sub_62a358)
                if bn.id != BusId::Player {
                    self.set_task(i, Task::SittingInBus, f);
                    return;
                }
                match p.ticket {
                    Ticket::Stamp => p.pt_target = bn.cabin.stamper.and_then(|d| d.0),
                    Ticket::Buy => p.pt_target = bn.cabin.sale.and_then(|d| d.0),
                    Ticket::None => self.route_to_place(i, bn),
                }
                let p = self.pax_mut(i).unwrap();
                if p.pt_target.is_none() {
                    // no path point at the device: straight on to the place
                    p.ticket = Ticket::None;
                    self.route_to_place(i, bn);
                }
            }
            Task::InBusToExit => {
                let p = self.pax(i).unwrap();
                let Some(bn) = f.bus(p.bus.or(p.inside)) else {
                    return;
                };
                if bn.id == BusId::Player {
                    self.stop_request = true;
                }
                let p = self.pax(i).unwrap();
                let from = p
                    .seat
                    .and_then(|k| bn.cabin.seats.get(k))
                    .map_or(p.pos.as_vec3(), |s| s.pos);
                let start =
                    bn.cabin
                        .omsi_nearest(from, &bn.cabin.all_points(), false, true, None, None);
                let exits = bn.cabin.exit_points();
                let p = self.pax_mut(i).unwrap();
                p.walking_in();
                p.pt = start;
                if let Some(q) = start.and_then(|k| bn.cabin.points.get(k)) {
                    p.pos = q.as_dvec3();
                }
                p.pt_target =
                    bn.cabin
                        .omsi_nearest(p.pos.as_vec3(), &exits, false, false, None, None);
                p.door = p
                    .pt_target
                    .and_then(|t| exits.iter().position(|e| *e == Some(t)));
                let (door, seat) = (p.door, p.seat.take());
                if let Some(d) = door {
                    self.request_door(bn.id, d, true);
                }
                if let Some(k) = seat {
                    self.free_seat(bn.id, k);
                }
            }
            Task::WalkingToBusstop => {
                let ride_km = self.ride_km();
                let p = self.pax_mut(i).unwrap();
                p.short = false;
                p.door = None;
                p.ride_km = ride_km;
                let (stop, spot) = (p.stop, p.spot);
                let spot = spot.or_else(|| stop.and_then(|s| self.take_spot(s)));
                let stop = stop.and_then(|s| self.stops.get(&s));
                let sp = stop.zip(spot).and_then(|(s, k)| s.spots.get(k).cloned());
                let stop_pos = stop.map(|s| s.pos);
                let p = self.pax_mut(i).unwrap();
                p.spot = spot;
                p.target_bus = false;
                match sp {
                    Some(sp) => {
                        // a seat: in front of it, the hip at its height (0x62e5d1)
                        let mut tgt = sp.pos;
                        if sp.height != 0.0 {
                            tgt.z = tgt.z.min(
                                (sp.pos.z - sp.height as f64).max(stop_pos.map_or(tgt.z, |s| s.z)),
                            );
                        }
                        p.target = tgt;
                        p.target_yaw = sp.face.to_radians();
                    }
                    None => {
                        p.target = stop_pos.unwrap_or(p.target);
                        p.target_yaw = 0.0;
                    }
                }
                p.st = Move::ToTarget;
            }
            Task::SittingInBus => {
                let Some(bn) = f.bus(self.pax(i).unwrap().inside) else {
                    return;
                };
                let seat = self
                    .pax(i)
                    .unwrap()
                    .seat
                    .and_then(|k| bn.cabin.seats.get(k));
                let p = self.pax_mut(i).unwrap();
                p.st = Move::Stand;
                if let Some(s) = seat {
                    p.take_seat(s, seat_height);
                }
                p.room = OUTSIDE_ROOM;
                p.reach = false;
                p.look_driver = false;
            }
            Task::Nothing => {}
        }
    }

    /// Every frame on the way: the nearest open entry or one with a button, one selling tickets
    /// for a buyer.
    fn choose_entry(&mut self, i: usize, f: &Frame) {
        let p = self.pax(i).unwrap();
        let Some(bn) = f.bus(p.bus) else {
            return;
        };
        let here = if p.inside.is_some() {
            p.pos.as_vec3()
        } else {
            bn.to_local(p.pos)
        };
        let list = bn.cabin.entry_points();
        let open: Vec<bool> = (0..list.len())
            .map(|k| bn.entry_open.get(k.min(7)).copied().unwrap_or(false))
            .collect();
        let pt = bn.cabin.omsi_nearest(
            here,
            &list,
            p.ticket == Ticket::Buy,
            false,
            Some(&bn.cabin.entry_flags()),
            Some(&open),
        );
        let p = self.pax_mut(i).unwrap();
        if let Some(q) = pt.and_then(|k| bn.cabin.points.get(k)) {
            p.target = q.as_dvec3();
            p.target_bus = true;
        }
        p.door = pt.and_then(|t| list.iter().position(|e| *e == Some(t)));
    }

    pub(super) fn route_to_place(&mut self, i: usize, bn: &BusNow) {
        let seat = self
            .pax(i)
            .unwrap()
            .seat
            .and_then(|k| bn.cabin.seats.get(k))
            .map(|s| s.pos);
        let p = self.pax_mut(i).unwrap();
        if let Some(s) = seat {
            p.pt_target = bn
                .cabin
                .omsi_nearest(s, &bn.cabin.all_points(), false, true, None, None);
        }
        p.st = Move::Path;
        p.smooth = false;
    }

    fn pax_task(
        &mut self,
        i: usize,
        f: &Frame,
        renderer: &Renderer,
        scene: &mut Scene,
        taken_ticket: &mut bool,
    ) {
        let p = self.pax(i).unwrap();
        let (task, st, stop, inside) = (p.task, p.st, p.stop, p.inside);
        let bn = f.bus(p.bus);
        match task {
            Task::WaitingForBus => {
                let Some(stop) = stop else { return };
                let Some(bn) = self.bus_for(i, stop, f).and_then(|b| f.bus(Some(b))) else {
                    return;
                };
                self.pax_mut(i).unwrap().bus = Some(bn.id);
                // still rolling in, or standing in the stop's box: to the gather point
                if bn.speed.abs() > 2.0 || self.in_stop_box(stop, bn.id) {
                    self.set_task(i, Task::ToBus, f);
                }
            }
            Task::ToBus => {
                let Some(stop) = stop else { return };
                if let Some(bn) =
                    bn.filter(|bn| bn.speed.abs() < 3.0 && self.in_stop_box(stop, bn.id))
                {
                    if let Some(k) = self.reserve_place(bn.id, bn.cabin.seats.len()) {
                        let (ticket, id) = self.decide_pax_ticket(i, bn);
                        let price = self
                            .tickets
                            .as_ref()
                            .and_then(|t| t.tickets.get(id.saturating_sub(1) as usize))
                            .map_or(0.0, |t| t.value);
                        let p = self.pax_mut(i).unwrap();
                        p.seat = Some(k);
                        p.ticket = ticket;
                        p.ticket_id = id;
                        p.price = if id > 0 { price } else { 0.0 };
                        self.set_task(i, Task::WalkingToBus, f);
                    }
                }
                let p = self.pax_mut(i).unwrap();
                p.short = true;
                let (bus, task) = (p.bus, p.task);
                if task == Task::ToBus && bus.is_none_or(|b| !self.listed_at(stop, b)) {
                    self.set_task(i, Task::WalkingToBusstop, f);
                }
            }
            Task::WalkingToBus => self.task_to_bus(i, f),
            Task::InBusToPlace => self.task_to_place(i, f, renderer, scene, taken_ticket),
            Task::InBusToExit => self.task_to_exit(i, f),
            Task::WalkingToBusstop => {
                if st == Move::AtTarget {
                    self.set_task(i, Task::WaitingForBus, f);
                } else {
                    self.pax_mut(i).unwrap().pax_state = 1;
                }
            }
            Task::SittingInBus => {
                let Some(b) = inside else { return };
                let reg = f.at_stops.get(&b);
                let km = self.odometer.get(&b).copied().unwrap_or(0.0);
                let p = self.pax(i).unwrap();
                let theirs = reg
                    .and_then(|r| r.next)
                    .and_then(|n| self.stops.get(&n))
                    .zip(p.dest.as_ref());
                if reg.is_some_and(|r| r.all_exit)
                    || theirs.is_some_and(|(s, dest)| s.is_named(dest))
                    || (p.dest.is_none() && p.km_start + (p.ride_km as f64) < km)
                {
                    self.set_task(i, Task::InBusToExit, f);
                }
            }
            Task::Nothing => {}
        }
    }

    fn task_to_bus(&mut self, i: usize, f: &Frame) {
        let p = self.pax(i).unwrap().clone();
        let Some(bn) = f.bus(p.bus) else {
            self.set_task(i, Task::WalkingToBusstop, f);
            return;
        };
        let door_open = |p: &Pax| {
            p.door
                .is_some_and(|d| bn.entry_open.get(d.min(7)).copied().unwrap_or(false))
        };
        let pp = self.pax_mut(i).unwrap();
        pp.clamp = true;
        pp.clamp_left = p
            .door
            .and_then(|d| bn.cabin.entries.get(d))
            .map_or(0.0, |e| e.inside.x)
            < 0.0;
        pp.clamp_x = if pp.clamp_left {
            bn.centre.x - bn.half.x - 0.5
        } else {
            bn.centre.x + bn.half.x + 0.5
        };
        pp.clamp_open = door_open(&p);
        // a shut door is asked for from the moment they stand at it
        if let (Move::AtTarget | Move::ShortOfTarget, Some(d)) = (p.st, p.door) {
            self.request_door(bn.id, d, false);
        }
        if p.seat.is_none() {
            self.set_task(i, Task::WalkingToBusstop, f);
            return;
        }
        let ok = bn.speed.abs() < 3.0
            && p.stop.is_some_and(|s| self.in_stop_box(s, bn.id))
            && !bn.cabin.points.is_empty();
        if ok && p.st != Move::AtTarget {
            self.choose_entry(i, f);
            let pp = self.pax_mut(i).unwrap();
            pp.short = !door_open(pp);
            pp.st = Move::ToTarget;
            pp.pax_state = 1;
        } else if ok {
            if bn.id == BusId::Player {
                self.greet_or_complain(i, bn);
            }
            self.set_task(i, Task::InBusToPlace, f);
        } else {
            if let Some(k) = p.seat {
                self.free_seat(bn.id, k);
            }
            self.pax_mut(i).unwrap().seat = None;
            let next = if bn.speed.abs() >= 3.0 {
                Task::ToBus
            } else {
                Task::WalkingToBusstop
            };
            self.set_task(i, next, f);
        }
    }

    fn task_to_place(
        &mut self,
        i: usize,
        f: &Frame,
        renderer: &Renderer,
        scene: &mut Scene,
        taken_ticket: &mut bool,
    ) {
        let p = self.pax(i).unwrap();
        let Some(bn) = f.bus(p.inside) else {
            return;
        };
        if p.st == Move::PathEnd {
            if p.ticket == Ticket::None {
                self.set_task(i, Task::SittingInBus, f);
                return;
            }
            let p = self.pax_mut(i).unwrap();
            if bn.id != BusId::Player {
                p.sub = 0;
                p.ticket = Ticket::None;
            } else {
                p.st = Move::Turn;
                p.smooth = true;
                let device = if p.ticket == Ticket::Stamp {
                    bn.cabin.stamper.map(|s| s.1)
                } else {
                    bn.cabin.money.map(|m| m.0)
                };
                if let Some(at) = device {
                    p.target = at.as_dvec3();
                    p.target_bus = true;
                    p.reach_at = at;
                }
                if p.ticket == Ticket::Stamp {
                    p.timer = 1.0;
                    p.reach = true;
                    p.sub = 1;
                } else {
                    p.sub = 3;
                }
            }
        }
        let p = self.pax(i).unwrap();
        match (p.ticket, p.sub) {
            (Ticket::Stamp, 1) if p.timer < 0.5 => {
                self.stamped.push(bn.id);
                let p = self.pax_mut(i).unwrap();
                p.sub = 2;
                p.reach = false;
            }
            (Ticket::Stamp, 2) if p.timer <= 0.0 => {
                self.route_to_place(i, bn);
                let p = self.pax_mut(i).unwrap();
                p.pt = bn.cabin.stamper.and_then(|s| s.0);
                p.ticket = Ticket::None;
                p.sub = 0;
            }
            (Ticket::Buy, _) => self.desk_sale(i, bn, f, renderer, scene, taken_ticket),
            _ => {}
        }
    }

    fn task_to_exit(&mut self, i: usize, f: &Frame) {
        let p = self.pax(i).unwrap().clone();
        let Some(bn) = f.bus(p.inside) else {
            return;
        };
        let at_stop = f.at_stops.get(&bn.id).and_then(|r| r.next);
        if bn.speed.abs() >= 1.0 {
            self.pax_mut(i).unwrap().timer = 1.0;
        }
        let door_open = p
            .door
            .is_some_and(|d| bn.exit_open.get(d.min(7)).copied().unwrap_or(false));
        let may_leave = door_open && (at_stop.is_some() || p.complaint == 3);
        self.pax_mut(i).unwrap().short = !may_leave;
        if !(p.st == Move::PathEnd && bn.speed.abs() < 1.0 && may_leave) {
            if bn.id == BusId::Player {
                self.stop_request = true;
            }
            if at_stop.is_none() {
                return;
            }
            if p.timer < 0.0 {
                // the bus stands: the nearest exit open now (0x62d6b1), once a second - found
                // every frame, whoever had left a point was pulled back to it (#493)
                let exits = bn.cabin.exit_points();
                let open: Vec<bool> = (0..exits.len())
                    .map(|k| bn.exit_open.get(k.min(7)).copied().unwrap_or(false))
                    .collect();
                let p = self.pax_mut(i).unwrap();
                p.timer = 1.0;
                let here = p.pos.as_vec3();
                let target = bn
                    .cabin
                    .omsi_nearest(here, &exits, false, false, None, Some(&open));
                if p.st == Move::Path {
                    // walking: on from the point walked to, towards the new door
                    p.pt_target = target;
                } else if target != p.pt_target || p.st != Move::PathEnd {
                    p.pt = bn.cabin.omsi_nearest(
                        here,
                        &bn.cabin.all_points(),
                        false,
                        false,
                        None,
                        None,
                    );
                    p.pt_target = target;
                    p.st = Move::Path;
                }
                p.door = p
                    .pt_target
                    .and_then(|t| exits.iter().position(|e| *e == Some(t)));
            }
            if let Some(d) = self.pax(i).unwrap().door {
                self.request_door(bn.id, d, true);
            }
            return;
        }
        let Some((w, h)) = self.pax_world(&p, f) else {
            return;
        };
        // the packs have no goodbye: their "thanks", now and then, never after a bad ride
        let chat = self.tickets.as_ref().map_or(0.0, |t| t.chattiness);
        if bn.id == BusId::Player
            && p.complaint < 3
            && (self.rand_f() as f32) < chat
            && !self.say(i, "Thanks_1", true)
        {
            self.say(i, "Thanks", true);
        }
        log::debug!(
            "{} gets off at stop {at_stop:?} by exit {:?}",
            self.people[i].label(),
            p.door
        );
        let p = self.pax_mut(i).unwrap();
        p.inside = None;
        p.pos = w;
        p.yaw = h.to_radians();
        self.walk_street(i, w, h, at_stop);
    }

    pub(super) fn walk_street(&mut self, i: usize, at: DVec3, heading: f64, stop: Option<i64>) {
        let lane = stop
            .and_then(|s| self.stops.get(&s))
            .and_then(|s| s.lane)
            .filter(|_| self.ped.is_some());
        let p = &mut self.people[i];
        p.position = at;
        p.heading = heading;
        p.place = Place::Ground;
        p.interior = 0.0;
        p.vel = DVec2::ZERO;
        p.state = match lane {
            Some((lane, s)) => {
                State::Strolling(PedWalk::new(vec![Leg { lane, a: s, b: s }], true, 0.0))
            }
            None => State::Standing,
        };
    }
}
