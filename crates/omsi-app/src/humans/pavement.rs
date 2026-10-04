use super::*;

/// Shoulders and swinging arms: with the cabin's radius people walking past merged.
const BODY_OUTSIDE: f64 = 0.28;
const STROLL_RADIUS: f64 = 200.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct Leg {
    pub(super) lane: usize,
    pub(super) a: f32,
    pub(super) b: f32,
}

impl Leg {
    fn len(&self) -> f32 {
        (self.b - self.a).abs()
    }

    pub(super) fn dist(&self, p: f32) -> f32 {
        if self.b >= self.a {
            self.a + p
        } else {
            self.a - p
        }
    }

    fn at(&self, net: &Network, p: f32) -> (DVec3, f64) {
        let (q, h) = net.lanes[self.lane].at(self.dist(p.clamp(0.0, self.len())));
        (
            q,
            if self.b >= self.a {
                h as f64
            } else {
                h as f64 + 180.0
            },
        )
    }

    fn project(&self, net: &Network, pos: DVec3, hint: f32) -> f32 {
        let (lo, hi) = ((hint - 1.5).max(0.0), (hint + 3.0).min(self.len()));
        let mut best = (hint, f64::MAX);
        let mut p = lo;
        while p <= hi + 1e-3 {
            let d = (self.at(net, p).0 - pos).truncate().length_squared();
            if d < best.1 {
                best = (p, d);
            }
            p += 0.2;
        }
        best.0
    }

    fn from_end(&self, net: &Network) -> bool {
        self.a < 0.05 || self.a > net.lanes[self.lane].length() - 0.05
    }

    fn reversed(&self) -> Leg {
        Leg {
            lane: self.lane,
            a: self.b,
            b: self.a,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PedWalk {
    pub(super) legs: Vec<Leg>,
    pub(super) leg: usize,
    pub(super) s: f32,
    roam: bool,
    side: f32,
    held: f32,
}

impl PedWalk {
    pub(super) fn new(legs: Vec<Leg>, roam: bool, side: f32) -> PedWalk {
        PedWalk {
            legs,
            leg: 0,
            s: 0.0,
            roam,
            side,
            held: 0.0,
        }
    }
}

pub(super) struct PedNet {
    ends: HashMap<usize, (usize, usize)>,
    out: Vec<Vec<(usize, bool)>>,
    crossings: HashMap<usize, Vec<DVec2>>,
    crossed: HashMap<usize, Vec<usize>>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    nodes: Vec<DVec3>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    built: usize,
}

fn cell50(p: DVec3) -> (i32, i32) {
    ((p.x / 50.0).floor() as i32, (p.y / 50.0).floor() as i32)
}

impl PedNet {
    fn build(net: &Network) -> PedNet {
        let mut p = PedNet {
            ends: HashMap::new(),
            out: Vec::new(),
            crossings: HashMap::new(),
            crossed: HashMap::new(),
            grid: HashMap::new(),
            nodes: Vec::new(),
            cells: HashMap::new(),
            built: 0,
        };
        p.extend(net);
        log::info!(
            "pavement network: {} paths, {} junctions",
            p.ends.len(),
            p.nodes.len()
        );
        p
    }

    fn extend(&mut self, net: &Network) -> usize {
        let from = self.built.min(net.lanes.len());
        let before = self.ends.len();
        for (i, l) in net.lanes.iter().enumerate().skip(from) {
            if l.kind == LaneKind::Street && from > 0 {
                // a new carriageway may cross paths that are in already
                self.crossings.clear();
            }
            if l.kind != LaneKind::Sidewalk || l.points.len() < 2 || l.length() < 0.3 {
                continue;
            }
            let a = self.node_of(l.start());
            let b = self.node_of(l.end());
            if a == b && l.length() < 3.0 {
                continue;
            }
            self.ends.insert(i, (a, b));
            self.out[a].push((i, true));
            self.out[b].push((i, false));
            let mut seen: Vec<(i32, i32)> = Vec::new();
            for c in l.points.iter().map(|p| cell50(*p)) {
                if !seen.contains(&c) {
                    seen.push(c);
                    self.grid.entry(c).or_default().push(i);
                }
            }
        }
        self.built = net.lanes.len();
        self.ends.len() - before
    }

    fn node_of(&mut self, p: DVec3) -> usize {
        let (cx, cy) = ((p.x / 1.5).floor() as i64, (p.y / 1.5).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &n in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if (self.nodes[n] - p).truncate().length() < 1.2
                        && (self.nodes[n].z - p.z).abs() < 2.5
                    {
                        return n;
                    }
                }
            }
        }
        self.nodes.push(p);
        self.out.push(Vec::new());
        self.cells
            .entry((cx, cy))
            .or_default()
            .push(self.nodes.len() - 1);
        self.nodes.len() - 1
    }

    pub(super) fn nearest(&self, net: &Network, p: DVec3, reach: f64) -> Option<(usize, f32, f64)> {
        let (cx, cy) = cell50(p);
        let mut cands: Vec<(usize, f32, f64)> = Vec::new();
        let mut seen = HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in self.grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if seen.insert(i) {
                        if let Some((s, d)) = net.lanes[i].nearest_point(p).filter(|x| x.1 < reach)
                        {
                            cands.push((i, s, d));
                        }
                    }
                }
            }
        }
        cands.sort_by(|a, b| a.2.total_cmp(&b.2));
        let first = cands.first().copied();
        cands
            .into_iter()
            .find(|&(i, s, _)| {
                let (q, _) = net.lanes[i].at(s);
                !crosses_street(net, p.truncate(), q.truncate())
                    && self.crossings.get(&i).is_none_or(|x| x.is_empty())
            })
            // on an island between carriageways: the nearest after all
            .or(first)
    }

    fn end_node(&self, net: &Network, leg: &Leg) -> Option<usize> {
        let (a, b) = *self.ends.get(&leg.lane)?;
        let len = net.lanes[leg.lane].length();
        if leg.b < 0.05 {
            Some(a)
        } else if leg.b > len - 0.05 {
            Some(b)
        } else {
            None
        }
    }

    fn next_leg(&self, net: &Network, node: usize, came: usize, pick: u64) -> Option<Leg> {
        let back = self.ends.get(&came).copied();
        let twin = |l: usize| {
            l == came
                || matches!((self.ends.get(&l), back), (Some(&(a, b)), Some((c, d)))
                    if a == d && b == c && (net.lanes[l].length() - net.lanes[came].length()).abs() < 1.0)
        };
        let list: Vec<(usize, bool)> = self
            .out
            .get(node)?
            .iter()
            .copied()
            .filter(|(l, _)| !twin(*l))
            .collect();
        let heading_in = {
            let l = &net.lanes[came];
            if back.is_some_and(|(a, _)| a == node) {
                wrap_heading(l.start_heading() as f64 + 180.0)
            } else {
                l.end_heading() as f64
            }
        };
        let leaving = |&(l, fwd): &(usize, bool)| {
            let lane = &net.lanes[l];
            if fwd {
                lane.start_heading() as f64
            } else {
                wrap_heading(lane.end_heading() as f64 + 180.0)
            }
        };
        let onward: Vec<(usize, bool)> = list
            .iter()
            .copied()
            .filter(|o| crowd::angle_diff(heading_in, leaving(o)).abs() <= 110.0)
            .collect();
        let list = if onward.is_empty() { list } else { onward };
        let (lane, fwd) = if list.is_empty() {
            // a dead end: turn round
            let (a, _) = back?;
            (came, a == node)
        } else {
            list[(pick as usize) % list.len()]
        };
        let len = net.lanes[lane].length();
        Some(if fwd {
            Leg {
                lane,
                a: 0.0,
                b: len,
            }
        } else {
            Leg {
                lane,
                a: len,
                b: 0.0,
            }
        })
    }

    fn crossings(&mut self, net: &Network, lane: usize) -> &[DVec2] {
        if !self.crossings.contains_key(&lane) {
            let l = &net.lanes[lane];
            let mut cand: Vec<usize> = Vec::new();
            for &i in l
                .points
                .iter()
                .filter_map(|p| net.grid.get(&cell50(*p)))
                .flatten()
            {
                if net.lanes[i].kind == LaneKind::Street && !cand.contains(&i) {
                    cand.push(i);
                }
            }
            let (mut out, mut crossed) = (Vec::new(), Vec::new());
            for i in cand {
                for w in l.points.windows(2) {
                    for v in net.lanes[i].points.windows(2) {
                        if (w[0].z - v[0].z).abs() > 3.0 {
                            continue;
                        }
                        let Some(x) = seg_cross(
                            w[0].truncate(),
                            w[1].truncate(),
                            v[0].truncate(),
                            v[1].truncate(),
                        ) else {
                            continue;
                        };
                        if !crossed.contains(&i) {
                            crossed.push(i);
                        }
                        if !out.iter().any(|q: &DVec2| (*q - x).length() < 1.5) {
                            out.push(x);
                        }
                    }
                }
            }
            self.crossings.insert(lane, out);
            self.crossed.insert(lane, crossed);
        }
        &self.crossings[&lane]
    }

    fn crossed_lanes(&mut self, net: &Network, lane: usize) -> &[usize] {
        self.crossings(net, lane);
        &self.crossed[&lane]
    }
}

fn crosses_street(net: &Network, a: DVec2, b: DVec2) -> bool {
    let mut cells: Vec<(i32, i32)> = Vec::new();
    for p in [a, b, (a + b) * 0.5] {
        let c = Network::grid_cell(p.extend(0.0));
        if !cells.contains(&c) {
            cells.push(c);
        }
    }
    let mut seen: Vec<usize> = Vec::new();
    for &i in cells.iter().filter_map(|c| net.grid.get(c)).flatten() {
        if seen.contains(&i) {
            continue;
        }
        seen.push(i);
        let l = &net.lanes[i];
        if l.kind == LaneKind::Street
            && l.points
                .windows(2)
                .any(|w| seg_cross(a, b, w[0].truncate(), w[1].truncate()).is_some())
        {
            return true;
        }
    }
    false
}

fn seg_cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<DVec2> {
    let (r, s) = (b - a, d - c);
    let den = r.perp_dot(s);
    if den.abs() < 1e-9 {
        return None;
    }
    let t = (c - a).perp_dot(s) / den;
    let u = (c - a).perp_dot(r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a + r * t)
}

fn pedestrian_window(
    ped: Option<&mut PedNet>,
    net: &Network,
    traffic: &Traffic,
    path: usize,
    green_left: f32,
) -> f32 {
    let mut window = f32::MAX;
    for &s in ped.map(|p| p.crossed_lanes(net, path)).unwrap_or(&[]) {
        let feeding = net.prev.get(s).map(|p| p.as_slice()).unwrap_or(&[]);
        for &l in std::iter::once(&s).chain(feeding) {
            if let Some(g) = net.lanes[l]
                .traffic_light
                .and_then(|(c, li)| traffic.light_until_go(c, li))
                .filter(|g| *g > 0.0 && *g >= green_left)
            {
                window = window.min(g);
            }
        }
    }
    if window == f32::MAX {
        green_left + 2.0
    } else {
        window
    }
}

fn arrive(from: DVec2, to: DVec2, pace: f64) -> DVec2 {
    let d = to - from;
    let dist = d.length();
    if dist < 0.1 {
        return DVec2::ZERO;
    }
    d / dist * (pace * dist.min(1.0)).max(if dist > 0.3 { 0.25 } else { 0.0 })
}

/// Cars for the crossings: position, velocity, half length.
type Car = (DVec2, DVec2, f64);

impl Humans {
    pub(super) fn update_pavements(&mut self, net: &Network) {
        let fresh = self.ped.is_none();
        let ped = self.ped.get_or_insert_with(|| PedNet::build(net));
        if !fresh && (ped.built >= net.lanes.len() || ped.extend(net) == 0) {
            return;
        }
        for s in self
            .stops
            .values_mut()
            .filter(|s| fresh || s.lane.is_none())
        {
            s.lane = ped.nearest(net, s.pos, 12.0).map(|(l, d, _)| (l, d));
        }
    }

    pub(super) fn populate_on_foot(
        &mut self,
        world: &World,
        net: &Network,
        renderer: &Renderer,
        scene: &mut Scene,
    ) {
        let center = self.center;
        let Some(ped) = self.ped.as_ref() else { return };
        let lanes: Vec<usize> = ped
            .ends
            .keys()
            .copied()
            .filter(|&l| {
                (net.lanes[l].start() - center).truncate().length() < STROLL_RADIUS * 0.9
                    && net.lanes[l].length() > 4.0
            })
            .collect();
        let crowd = (lanes.len() as f32 / 120.0).clamp(0.6, 3.0);
        let target =
            (self.pedestrians as f32 * crowd * self.density.clamp(0.0, 3.0)).round() as usize;
        // (around this player only, when a LAN host keeps people around several)
        let have = self
            .people
            .iter()
            .filter(|p| matches!(p.state, State::Strolling(_)))
            .filter(|p| {
                self.lan_centers.is_empty() || (p.position - center).length() < STROLL_RADIUS
            })
            .count();
        if have >= target || lanes.is_empty() {
            return;
        }
        for _ in 0..(target - have).min(4) {
            let lane = lanes[(self.rand() as usize) % lanes.len()];
            let len = net.lanes[lane].length();
            let s = (self.rand_f() as f32 * (len - 1.0)).max(0.5);
            let (p, h) = net.lanes[lane].at(s);
            if self.seen(p)
                || (p - center).length() < 20.0
                || (p - center).length() > STROLL_RADIUS * 0.9
                || !world.has_ground(p.x, p.y)
            {
                continue;
            }
            let fwd = self.rand_f() < 0.5;
            let leg = Leg {
                lane,
                a: s,
                b: if fwd { len } else { 0.0 },
            };
            let side = 0.3 + self.rand_f() as f32 * 0.4;
            let heading = if fwd { h as f64 } else { h as f64 + 180.0 };
            let walk = State::Strolling(PedWalk::new(vec![leg], true, side));
            if let Some(i) = self.spawn(world, renderer, scene, p, heading, walk) {
                self.people[i].activity = Activity::Walk;
            }
        }
    }

    pub(super) fn walk_pedestrians(
        &mut self,
        dt: f32,
        f: &Frame,
        net: Option<&Network>,
        traffic: Option<&Traffic>,
        remove: &mut Vec<usize>,
    ) {
        let (cars, blocks) = self.obstacles(f.world, traffic, f.bus(Some(BusId::Player)));
        let mut wants: Vec<Want> = Vec::with_capacity(self.people.len());
        for i in 0..self.people.len() {
            let p = &self.people[i];
            wants.push(
                if p.avatar || p.remote || matches!(p.state, State::Pax(_)) {
                    Want::stand(None)
                } else {
                    self.decide(i, dt, f.world, net, traffic, &cars, remove)
                },
            );
        }
        // a standing vehicle in the way: wait, then go round it
        for (i, want) in wants.iter_mut().enumerate() {
            let p = &mut self.people[i];
            if remove.contains(&i)
                || p.avatar
                || p.remote
                || !matches!(p.state, State::Strolling(_))
            {
                continue;
            }
            let speed = want.vel.length();
            let ahead = p.position.truncate() + want.vel / speed * 0.9;
            let in_way = speed >= 0.2
                && blocks.iter().any(|b| {
                    b.vel.length() < 0.5 && b.near(ahead, BODY_OUTSIDE + 0.15) && {
                        let (q, inside) = b.closest(ahead);
                        inside || (ahead - q).length() < BODY_OUTSIDE + 0.15
                    }
                });
            if !in_way {
                p.car_wait = 0.0;
                continue;
            }
            p.car_wait += dt;
            if p.car_wait > 8.0 {
                p.detour = p.detour.max(4.0);
                p.car_wait = 0.0;
            } else if p.detour <= 0.0 {
                want.vel = DVec2::ZERO;
            }
        }
        let mut walkers: Vec<Walker> = Vec::with_capacity(self.people.len());
        let mut who: Vec<usize> = Vec::with_capacity(self.people.len());
        for (i, p) in self.people.iter().enumerate() {
            if remove.contains(&i) || p.avatar || p.remote || p.place != Place::Ground {
                continue;
            }
            let w = &wants[i];
            walkers.push(Walker {
                pos: p.position.truncate(),
                vel: p.vel,
                radius: BODY_OUTSIDE,
                want: w.vel,
                give: w.give,
                // passengers outside stand in the crowd as they are
                fixed: matches!(p.state, State::Pax(_)),
                ghost: p.ghost > 0.0,
                corridor: if p.detour > 0.0 { None } else { w.corridor },
            });
            who.push(i);
        }
        let near_blocks: Vec<Block> = blocks
            .into_iter()
            .filter(|b| walkers.iter().any(|w| b.near(w.pos, 25.0)))
            .collect();
        crowd::step(
            &mut walkers,
            &near_blocks,
            &CrowdParams::default(),
            dt as f64,
        );
        self.keep_out_of_walls(f.world, &who, &mut walkers);
        let mut moved = vec![false; self.people.len()];
        for (k, w) in walkers.iter().enumerate() {
            let i = who[k];
            if !matches!(self.people[i].state, State::Pax(_)) {
                moved[i] = true;
                self.apply(i, w, &wants[i], dt, f.world, net);
            }
        }
        for i in 0..self.people.len() {
            if !moved[i] && !matches!(self.people[i].state, State::Pax(_)) {
                self.carry(i, dt, f, wants[i].face);
            }
        }
    }

    fn obstacles(
        &self,
        world: &World,
        traffic: Option<&Traffic>,
        player: Option<&BusNow>,
    ) -> (Vec<Car>, Vec<Block>) {
        let (mut cars, mut blocks) = (Vec::new(), Vec::new());
        let dir = |deg: f64| DVec2::new(deg.to_radians().sin(), deg.to_radians().cos());
        for c in traffic.map(|t| t.cars.as_slice()).unwrap_or(&[]) {
            let v = &c.vehicle;
            if (v.position - self.center).length() > 320.0 {
                continue;
            }
            let speed = c.state.speed as f64;
            let bb =
                v.ty.def
                    .bounding_box
                    .unwrap_or([2.0, 4.5, 1.6, 0.0, 0.0, 0.8]);
            cars.push((
                v.position.truncate(),
                dir(v.heading) * speed,
                bb[1] as f64 * 0.5,
            ));
            if matches!(v.ty.def.kind, omsi_vehicle::VehicleKind::Other(3)) {
                continue;
            }
            let mut block = |bb, pos, heading| {
                let o = omsi_sim::collision::Obb::from_box(bb, pos, heading);
                blocks.push(Block {
                    center: o.center,
                    half: o.half,
                    heading: o.heading,
                    vel: dir(heading) * speed,
                });
            };
            block(bb, v.position, v.heading);
            for t in &v.trailers {
                block(
                    t.ty.def
                        .bounding_box
                        .unwrap_or([2.5, 7.0, 3.0, 0.0, 0.0, 1.5]),
                    t.position,
                    t.heading,
                );
            }
        }
        for o in world.parked_boxes.lock().iter() {
            if (o.center - self.center.truncate()).length() < 320.0 {
                blocks.push(Block {
                    center: o.center,
                    half: o.half,
                    heading: o.heading,
                    vel: DVec2::ZERO,
                });
            }
        }
        if let Some(pb) = player {
            cars.push((pb.pos.truncate(), pb.fwd() * pb.speed, pb.half.y));
            cars.extend(
                pb.trailers
                    .iter()
                    .map(|t| (t.pos.truncate(), dir(t.heading) * pb.speed, t.half.y)),
            );
            blocks.extend(pb.blocks());
        }
        (cars, blocks)
    }

    fn keep_out_of_walls(&mut self, world: &World, who: &[usize], walkers: &mut [Walker]) {
        const R: f64 = 0.22;
        const CELL: f64 = 12.0;
        let collision = world.collision.lock();
        let places: Vec<DVec2> = world
            .waiting_places
            .lock()
            .iter()
            .map(|w| w.1.truncate())
            .collect();
        let key = (collision.boxes.len(), collision.meshes.len(), places.len());
        if key != (self.wall_key.0, self.wall_key.1, self.wall_key.2)
            || self.time - self.wall_key.3 > 2.0
            || self.time < self.wall_key.3
        {
            self.wall_cells.clear();
            self.wall_key = (key.0, key.1, key.2, self.time);
        }
        let mut cells = std::mem::take(&mut self.wall_cells);
        for (k, w) in walkers.iter_mut().enumerate() {
            let i = who[k];
            if w.fixed || self.people[i].place != Place::Ground {
                continue;
            }
            let p0 = self.people[i].position.truncate();
            if (w.pos - p0).length_squared() < 1e-8 {
                continue;
            }
            let z = self.people[i].position.z;
            let key = (
                (p0.x / CELL).floor() as i32,
                (p0.y / CELL).floor() as i32,
                z.floor() as i32,
            );
            let walls = cells.entry(key).or_insert_with(|| {
                let c = DVec2::new((key.0 as f64 + 0.5) * CELL, (key.1 as f64 + 0.5) * CELL);
                let probe = omsi_sim::collision::Obb {
                    center: c,
                    half: DVec2::splat(CELL * 0.5 + 2.0),
                    heading: 0.0,
                    z0: key.2 as f64 - 1.0,
                    z1: key.2 as f64 + 3.5,
                    velocity: DVec2::ZERO,
                    mass: 0.0,
                    pole: None,
                    id: -1,
                };
                let near = collision.obstacles_near(&probe);
                let reach = near
                    .iter()
                    .map(|o| (o.center - c).length() + o.half.length() + 1.0)
                    .fold(0.0, f64::max);
                let local: Vec<DVec2> = places
                    .iter()
                    .copied()
                    .filter(|q| (*q - c).length() < reach)
                    .collect();
                near.into_iter()
                    // a shelter given as one solid box has its waiting places inside
                    .filter(|o| {
                        let b = Block {
                            center: o.center,
                            half: o.half,
                            heading: o.heading,
                            vel: DVec2::ZERO,
                        };
                        !local.iter().any(|q| {
                            (*q - o.center).length() < o.half.length() + 1.0 && b.near(*q, 0.3)
                        })
                    })
                    .map(|o| {
                        let b = Block {
                            center: o.center,
                            half: o.half + DVec2::splat(R),
                            heading: o.heading,
                            vel: DVec2::ZERO,
                        };
                        (b, o.z0, o.z1)
                    })
                    .collect()
            });
            for (b, z0, z1) in walls.iter() {
                if *z0 > z + 1.6 || *z1 < z + 0.5 {
                    continue;
                }
                if (w.pos - b.center).length_squared() >= b.half.length_squared()
                    || !b.near(w.pos, 0.0)
                    || b.near(p0, -0.01)
                {
                    continue;
                }
                let (q, inside) = b.closest(w.pos);
                if !inside {
                    continue;
                }
                let n = (q - w.pos).try_normalize().unwrap_or(DVec2::ZERO);
                w.pos = q + n * 0.005;
                let vn = w.vel.dot(n);
                if vn < 0.0 {
                    w.vel -= n * vn;
                }
                let p = &mut self.people[i];
                let fresh = p.detour <= 0.0;
                p.detour = 2.0;
                w.corridor = None;
                // straight at it: round it the way that turns least (a lamp post stopped people
                // dead), and keep to that way (the two sides' choices flipped each other)
                let speed = w.want.length();
                let t = DVec2::new(-n.y, n.x);
                if fresh || p.detour_side == 0.0 {
                    let along = w.want.dot(t);
                    p.detour_side = if along.abs() > 0.05 * speed {
                        along.signum()
                    } else if i % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                }
                if speed > 0.2 && w.vel.dot(t) * p.detour_side < 0.4 * speed {
                    w.vel = t * p.detour_side * speed * 0.8;
                }
            }
        }
        self.wall_cells = cells;
    }

    #[allow(clippy::too_many_arguments)]
    fn decide(
        &mut self,
        i: usize,
        dt: f32,
        world: &World,
        net: Option<&Network>,
        traffic: Option<&Traffic>,
        cars: &[Car],
        remove: &mut Vec<usize>,
    ) -> Want {
        let p = &self.people[i];
        let pos = p.position;
        // walking on towards ground that is not loaded: gone
        if p.place == Place::Ground
            && p.vel.length_squared() > 1e-4
            && !world.has_ground(pos.x, pos.y)
        {
            remove.push(i);
            return Want::stand(None);
        }
        match p.state.clone() {
            State::Strolling(mut walk) => {
                let gone = !self.seen(pos) && self.far_from_players(pos, STROLL_RADIUS * 2.0);
                let Some(net) = net.filter(|_| !gone) else {
                    remove.push(i);
                    return Want::stand(None);
                };
                let mut ped = self.ped.take();
                let w = self.walk_want(ped.as_mut(), i, &mut walk, net, traffic, cars, dt);
                self.ped = ped;
                self.people[i].state = State::Strolling(walk);
                w
            }
            State::Standing => {
                if !self.seen(pos) && self.far_from_players(pos, STROLL_RADIUS) {
                    remove.push(i);
                }
                Want::stand(None)
            }
            _ => Want::stand(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_want(
        &mut self,
        mut ped: Option<&mut PedNet>,
        i: usize,
        walk: &mut PedWalk,
        net: &Network,
        traffic: Option<&Traffic>,
        cars: &[Car],
        dt: f32,
    ) -> Want {
        let pos2 = self.people[i].position.truncate();
        let pace = self.people[i].pace;
        let Some(&leg) = walk.legs.get(walk.leg) else {
            return Want::stand(None);
        };
        if walk.s >= leg.len() - 0.35 || walk.held > 0.0 {
            if walk.leg + 1 >= walk.legs.len() {
                if !walk.roam {
                    walk.leg += 1;
                    return Want::stand(None);
                }
                let pick = self.rand();
                let next = ped
                    .as_ref()
                    .and_then(|p| {
                        p.end_node(net, &leg)
                            .and_then(|n| p.next_leg(net, n, leg.lane, pick))
                    })
                    .unwrap_or(leg.reversed());
                walk.legs.push(next);
                if walk.leg > 6 {
                    walk.legs.drain(..walk.leg);
                    walk.leg = 0;
                }
            }
            let next = walk.legs[walk.leg + 1];
            match self.may_cross(
                ped.as_deref_mut(),
                net,
                &next,
                traffic,
                cars,
                pace,
                walk.held,
            ) {
                Ok(()) => {
                    walk.s = (walk.s - leg.len()).max(0.0);
                    walk.leg += 1;
                    walk.held = 0.0;
                }
                Err(why) => {
                    walk.held += dt;
                    self.people[i].why = why;
                    // a light that stays red: a stroller gives up after three minutes and turns back
                    if walk.roam && why == "red light" && walk.held > 180.0 {
                        log::debug!("{} gives up at a red light", self.people[i].label());
                        walk.legs.truncate(walk.leg + 1);
                        walk.legs.push(leg.reversed());
                        walk.held = 0.0;
                        return Want::stand(None);
                    }
                    // at the kerb facing the way across, spread along it and a step back
                    let (end, _) = leg.at(net, leg.len());
                    let (_, h) = next.at(net, 0.3);
                    let hr = h.to_radians();
                    let (fwd, right) = (
                        DVec2::new(hr.sin(), hr.cos()),
                        DVec2::new(hr.cos(), -hr.sin()),
                    );
                    let id = self.people[i].id;
                    let spot = end.truncate() + right * (((id % 5) as f64 - 2.0) * 0.45)
                        - fwd * (0.25 + (id % 3) as f64 * 0.45);
                    return Want {
                        vel: arrive(pos2, spot, pace * 0.6),
                        face: Some(h),
                        give: 0.5,
                        corridor: None,
                    };
                }
            }
        }
        let leg = walk.legs[walk.leg];
        let len = leg.len();
        let (p, h) = leg.at(net, (walk.s + 1.3).min(len));
        let lane = &net.lanes[leg.lane];
        let width = (lane.width as f64).max(1.0);
        let crossing = lane.traffic_light.is_some()
            || ped
                .as_ref()
                .is_some_and(|p| p.crossings.get(&leg.lane).is_some_and(|x| !x.is_empty()));
        // keep to the right of the pavement (less so on a crossing), the left where traffic does
        let side = if crossing {
            (walk.side.abs() as f64).min(0.3)
        } else {
            (walk.side.abs() as f64).min(width * 0.5 - 0.3).max(0.0)
        };
        let side = if net.left_hand { -side } else { side };
        let hr = h.to_radians();
        let right = DVec2::new(hr.cos(), -hr.sin());
        let vel = (p.truncate() + right * side - pos2).normalize_or_zero() * pace;
        let (a, _) = leg.at(net, (walk.s - 2.0).max(0.0));
        let (b, _) = leg.at(net, (walk.s + 2.5).min(len));
        let (m, _) = leg.at(net, (walk.s + 0.25).min(len));
        let bow = crowd::project_on_segment(m.truncate(), a.truncate(), b.truncate())
            .0
            .distance(m.truncate());
        let corridor = ((b - a).truncate().length() > 0.5).then(|| {
            (
                a.truncate() + right * side,
                b.truncate() + right * side,
                (width * 0.5 - side).max(0.35) + bow,
            )
        });
        if self.people[i].why != "queueing behind somebody" {
            self.people[i].why = "";
        }
        Want {
            vel,
            face: None,
            give: 1.0,
            corridor,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn may_cross(
        &self,
        mut ped: Option<&mut PedNet>,
        net: &Network,
        next: &Leg,
        traffic: Option<&Traffic>,
        cars: &[Car],
        pace: f64,
        held: f32,
    ) -> Result<(), &'static str> {
        if !next.from_end(net) {
            return Ok(());
        }
        let lane = &net.lanes[next.lane];
        let t_cross = next.len() as f64 / pace.max(0.5) + 1.0;
        if let (Some((c, li)), Some(t)) = (lane.traffic_light, traffic) {
            if let Some((state, left)) = t.light_state(c, li) {
                if !omsi_sim::traffic::TrafficLightController::allows_go(state) {
                    return Err("red light");
                }
                if held > 150.0 {
                    return Ok(());
                }
                // the green alone is short (8 s for an 11.6 m crossing): who starts on green has
                // the clearance after it, until the cars get theirs
                let window = pedestrian_window(ped.as_deref_mut(), net, t, next.lane, left);
                return if (window as f64) < t_cross {
                    Err("the green ends before they would be across")
                } else {
                    Ok(())
                };
            }
        }
        let Some(ped) = ped else { return Ok(()) };
        // a long wait accepts a shorter gap, never shorter than the crossing takes
        let margin = if held > 45.0 {
            0.0
        } else if held > 20.0 {
            1.0
        } else {
            2.5
        };
        for x in ped.crossings(net, next.lane) {
            for (p, v, half) in cars {
                let rel = *x - *p;
                let dist = rel.length();
                if dist > 90.0 {
                    continue;
                }
                if dist < half + 1.5 {
                    return Err("a vehicle stands on the crossing");
                }
                let speed = v.length();
                if speed < 0.5 {
                    continue;
                }
                let dir = *v / speed;
                let along = rel.dot(dir);
                if along > -half
                    && rel.perp_dot(dir).abs() < 3.5
                    && along / speed < t_cross + margin
                {
                    return Err("waits for a car to pass");
                }
            }
        }
        Ok(())
    }

    fn apply(
        &mut self,
        i: usize,
        w: &Walker,
        want: &Want,
        dt: f32,
        world: &World,
        net: Option<&Network>,
    ) {
        let p = &mut self.people[i];
        let speed = w.vel.length();
        // somebody pressed against somebody else for seconds slips past them
        if want.vel.length() > 0.2 && speed < 0.08 {
            p.stuck += dt;
        } else if speed > 0.2 {
            p.stuck = 0.0;
        }
        p.detour = (p.detour - dt).max(0.0);
        if p.ghost > 0.0 {
            p.ghost -= dt;
        } else if p.stuck > 2.5 {
            p.ghost = 1.5;
            p.stuck = 0.0;
        }
        p.vel = w.vel;
        p.position.x = w.pos.x;
        p.position.y = w.pos.y;
        if let Some(z) = world.walk_height_near(p.position.x, p.position.y, p.position.z) {
            // up a kerb quickly, down it smoothly; more than a kerb below is a pavement that
            // came after them, and whoever stands still simply stands on it
            p.position.z = if z - p.position.z > 0.35 || speed < 0.05 {
                z
            } else if z > p.position.z {
                z.min(p.position.z + 1.5 * dt as f64)
            } else {
                z.max(p.position.z - 2.0 * dt as f64)
            };
        }
        if let (Some(net), State::Strolling(walk)) = (net, &mut p.state) {
            if let Some(leg) = walk.legs.get(walk.leg) {
                walk.s = leg.project(net, p.position, walk.s).max(walk.s - 0.3);
            }
        }
        let walking = speed
            > if p.activity == Activity::Walk {
                0.12
            } else {
                0.3
            };
        let target = if speed > 0.25 {
            Some(crowd::heading_of(w.vel))
        } else {
            want.face
        };
        // turning eases in and out, up to the most a walker or a stander turns
        if let Some(t) = target {
            let rate = (crowd::angle_diff(p.heading, t).abs() * 5.0)
                .min(if walking { 260.0 } else { 140.0 })
                .max(12.0);
            p.heading = crowd::turn_towards(p.heading, t, rate, dt as f64);
        }
        p.activity = if walking {
            Activity::Walk
        } else {
            Activity::Stand
        };
    }

    fn carry(&mut self, i: usize, dt: f32, f: &Frame, face: Option<f64>) {
        let p = &mut self.people[i];
        let Place::Bus(b, l) = p.place else { return };
        let Some(bn) = f.bus(Some(b)) else {
            return;
        };
        p.position = bn.world(l);
        p.tilt = bn.tilt_at(l);
        p.vel = DVec2::ZERO;
        if let Some(f) = face {
            p.lheading = crowd::turn_towards(p.lheading, f, 150.0, dt as f64);
        }
        p.heading = bn.heading_at(l) + p.lheading;
        p.interior = bn.interior;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lane(points: [(f64, f64); 2], kind: LaneKind) -> omsi_sim::traffic::Lane {
        let points = points.iter().map(|&(x, y)| DVec3::new(x, y, 0.0)).collect();
        omsi_sim::traffic::LaneBuilder::polyline(points, kind, 2.5)
    }

    #[test]
    fn pavement_corners_are_joined_and_routed() {
        // an L of pavement, which the road network's heading rule leaves unlinked
        let mut net = Network::default();
        net.lanes
            .push(lane([(0.0, 0.0), (0.0, 20.0)], LaneKind::Sidewalk));
        net.lanes
            .push(lane([(0.5, 20.3), (30.0, 20.3)], LaneKind::Sidewalk));
        net.lanes
            .push(lane([(-5.0, 10.0), (5.0, 10.0)], LaneKind::Street));
        net.link(1.5);
        assert!(
            net.lanes[0].next.is_empty(),
            "the road rule does not join the corner"
        );
        let ped = PedNet::build(&net);
        let corner = ped
            .end_node(
                &net,
                &Leg {
                    lane: 0,
                    a: 5.0,
                    b: 20.0,
                },
            )
            .unwrap();
        assert!(
            ped.out[corner].iter().any(|&(l, fwd)| l == 1 && fwd),
            "{:?}",
            ped.out[corner]
        );
        // a dead end turns round
        let n = ped
            .end_node(
                &net,
                &Leg {
                    lane: 1,
                    a: 0.0,
                    b: net.lanes[1].length(),
                },
            )
            .unwrap();
        let turn = ped.next_leg(&net, n, 1, 7).unwrap();
        assert_eq!(turn.lane, 1);
        assert!(turn.a > turn.b);
    }

    #[test]
    fn crossings_of_a_pavement_path_are_found() {
        let mut net = Network::default();
        net.lanes
            .push(lane([(0.0, 0.0), (0.0, 8.0)], LaneKind::Sidewalk));
        net.lanes
            .push(lane([(-30.0, 4.0), (30.0, 4.0)], LaneKind::Street));
        net.link(1.5);
        let mut ped = PedNet::build(&net);
        let x = ped.crossings(&net, 0).to_vec();
        assert_eq!(x.len(), 1);
        assert!((x[0] - DVec2::new(0.0, 4.0)).length() < 1e-6);
        assert!(crosses_street(
            &net,
            DVec2::new(1.0, 0.0),
            DVec2::new(1.0, 8.0)
        ));
        assert!(!crosses_street(
            &net,
            DVec2::new(1.0, 0.0),
            DVec2::new(1.0, 3.0)
        ));
    }

    #[test]
    fn legs_run_both_ways() {
        let mut net = Network::default();
        net.lanes
            .push(lane([(0.0, 0.0), (0.0, 10.0)], LaneKind::Sidewalk));
        let back = Leg {
            lane: 0,
            a: 8.0,
            b: 2.0,
        };
        let (p, h) = back.at(&net, 1.0);
        assert!((p.y - 7.0).abs() < 1e-6);
        assert!((h - 180.0).abs() < 1e-6);
        assert!((back.project(&net, DVec3::new(0.3, 5.0, 0.0), 2.5) - 3.0).abs() < 0.11);
    }
}
