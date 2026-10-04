use super::*;

#[derive(Default)]
pub(super) struct Avatars {
    ids: HashMap<u32, u32>,
    cmds: HashMap<u32, AvatarCmd>,
    pub(super) hidden: HashMap<u32, bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct AvatarCmd {
    pub pos: DVec3,
    pub heading: f64,
    pub vel: DVec2,
    pub lift: f64,
    pub seat: Option<(BusId, usize)>,
    pub floor: Option<f64>,
    pub aboard: Option<(BusId, Vec3)>,
}

#[derive(Debug, Clone, Copy)]
pub struct SeatSpot {
    pub bus: BusId,
    pub seat: usize,
}

impl Humans {
    fn bus_now(&self, bus: BusId) -> Option<&BusNow> {
        self.last_buses.iter().find(|b| b.id == bus)
    }

    fn free_seats<'a>(&'a self, bn: &'a BusNow) -> impl Iterator<Item = (usize, &'a Seat)> + 'a {
        let taken = self.seats.get(&bn.id);
        bn.cabin.seats.iter().enumerate().filter(move |(k, s)| {
            s.seated && !taken.and_then(|t| t.get(*k)).copied().unwrap_or(false)
        })
    }

    pub fn avatar(
        &mut self,
        key: u32,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        cmd: AvatarCmd,
        kind: u64,
    ) {
        let known = self
            .avatars
            .ids
            .get(&key)
            .is_some_and(|id| self.people.iter().any(|p| p.id == *id));
        if !known {
            let n = self.types.len().max(1) as u64;
            let Some(i) = self.spawn_as(
                world,
                renderer,
                scene,
                cmd.pos,
                cmd.heading,
                State::Idle,
                Some((kind % n) as usize),
            ) else {
                return;
            };
            self.people[i].avatar = true;
            self.avatars.ids.insert(key, self.people[i].id);
        }
        // a seat taken is kept from the passengers; one left is theirs again
        let before = self.avatars.cmds.get(&key).and_then(|c| c.seat);
        if before != cmd.seat {
            if let Some((b, k)) = before {
                self.free_seat(b, k);
            }
            if let Some(t) = cmd
                .seat
                .and_then(|(b, k)| self.seats.get_mut(&b)?.get_mut(k))
            {
                *t = true;
            }
        }
        self.avatars.cmds.insert(key, cmd);
    }

    pub fn avatar_remove(&mut self, key: u32) {
        if let Some((b, k)) = self.avatars.cmds.remove(&key).and_then(|c| c.seat) {
            self.free_seat(b, k);
        }
        if let Some(i) = self
            .avatars
            .ids
            .remove(&key)
            .and_then(|id| self.people.iter().position(|p| p.id == id))
        {
            self.remove_person(i);
        }
    }

    pub fn avatar_show(&mut self, key: u32, show: bool) {
        if let Some(id) = self.avatars.ids.get(&key) {
            self.avatars.hidden.insert(*id, !show);
        }
    }

    pub fn avatar_body(&self, key: u32) -> Option<(DVec3, f64, DVec3)> {
        let id = self.avatars.ids.get(&key)?;
        let p = self.people.iter().find(|p| p.id == *id)?;
        let rig = &p.ty.rig;
        let eye_h = (rig.head_top - 0.11 * rig.scale) as f64;
        let eye = match (p.place, self.avatars.cmds.get(&key).and_then(|c| c.seat)) {
            (Place::Bus(b, _), Some((_, k))) => {
                let bn = self.bus_now(b)?;
                let s = bn.cabin.seats.get(k)?;
                // sitting: the eyes over the hip, a little back
                let r = s.rot.to_radians();
                bn.world(
                    s.pos
                        + Vec3::new(
                            -r.sin() * 0.05,
                            -r.cos() * 0.05,
                            (eye_h - rig.hip[0].z as f64) as f32 + 0.04,
                        ),
                )
            }
            _ => p.position + DVec3::new(0.0, 0.0, eye_h),
        };
        Some((p.position, p.heading, eye))
    }

    pub fn seat_near(&self, at: DVec3, reach: f64, only: Option<BusId>) -> Option<SeatSpot> {
        let mut best: Option<(f64, SeatSpot)> = None;
        for bn in self
            .last_buses
            .iter()
            .filter(|bn| only.is_none_or(|o| o == bn.id))
        {
            let Some(door) = bn
                .cabin
                .entries
                .iter()
                .chain(&bn.cabin.exits)
                .map(|d| bn.world(d.outside))
                .min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length()))
            else {
                continue;
            };
            let d = (door - at).truncate().length();
            if d > reach {
                continue;
            }
            let seat = self
                .free_seats(bn)
                .min_by(|a, b| {
                    (bn.world(a.1.floor) - door)
                        .length()
                        .total_cmp(&(bn.world(b.1.floor) - door).length())
                })
                .map(|(k, _)| k);
            if let Some(seat) = seat.filter(|_| best.is_none_or(|b| d < b.0)) {
                best = Some((d, SeatSpot { bus: bn.id, seat }));
            }
        }
        best.map(|b| b.1)
    }

    pub fn bus_doors(&self, bus: BusId) -> Vec<DVec3> {
        self.bus_now(bus)
            .map(|bn| {
                bn.cabin
                    .entries
                    .iter()
                    .chain(&bn.cabin.exits)
                    .map(|d| bn.world(d.outside))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn vehicle_driver_door(&mut self, v: &VehicleInstance) -> Option<DVec3> {
        // a van's own cab door first: its driver does not climb in through the sliding door
        if let Some(d) = self.vehicle_cab_door(v) {
            return Some(d);
        }
        let cabin = self.cabin_for(v)?;
        let seat = cabin
            .data
            .driver_positions
            .first()
            .map_or(Vec3::new(-0.8, 4.5, 1.0), |d| Vec3::from(d.pos));
        let flat = |d: &&Door| (d.outside - seat).truncate().length();
        let door = cabin
            .entries
            .iter()
            .chain(&cabin.exits)
            .min_by(|a, b| flat(a).total_cmp(&flat(b)))?;
        Some(train_point(
            v.position,
            &v.body_rotation(),
            &part_frames(v, &cabin),
            door.outside,
        ))
    }

    pub fn vehicle_cab_door(&mut self, v: &VehicleInstance) -> Option<DVec3> {
        let cabin = self.cabin_for(v)?;
        let seat = Vec3::from(cabin.data.driver_positions.first()?.pos);
        let flat = |d: &&Door| (d.outside - seat).truncate().length();
        let door = cabin
            .entries
            .iter()
            .chain(&cabin.exits)
            .filter(|d| d.outside.x * seat.x > 0.0 && (d.outside.y - seat.y).abs() < 1.5)
            .min_by(|a, b| flat(a).total_cmp(&flat(b)))
            .map(|d| d.outside)
            // a van whose cabin knows only the sliding door (the W906): the door beside the seat
            .or_else(|| {
                let bb = v.ty.def.bounding_box?;
                (bb[1] < 8.5 && seat.x.abs() > 0.2).then(|| {
                    Vec3::new(
                        seat.x.signum() * (bb[0] * 0.5 + bb[3] * seat.x.signum() + 0.45),
                        seat.y,
                        0.0,
                    )
                })
            })?;
        Some(train_point(
            v.position,
            &v.body_rotation(),
            &part_frames(v, &cabin),
            door,
        ))
    }

    pub fn vehicle_doors(&mut self, v: &VehicleInstance) -> Vec<DVec3> {
        let Some(cabin) = self.cabin_for(v) else {
            return Vec::new();
        };
        let (trailers, rot) = (part_frames(v, &cabin), v.body_rotation());
        cabin
            .entries
            .iter()
            .chain(&cabin.exits)
            .map(|d| train_point(v.position, &rot, &trailers, d.outside))
            .collect()
    }

    /// Kept within 0.3 m of the cabin's paths, on its floor and out of the seats.
    pub fn cabin_walk(&self, bus: BusId, local: Vec3, step: glam::Vec2) -> Option<(Vec3, DVec3)> {
        const WIDTH: f32 = 0.3;
        let bn = self.bus_now(bus)?;
        let pts = &bn.cabin.points;
        let want = local.truncate() + step;
        let mut best: Option<(f32, glam::Vec2, f32)> = None;
        for &(a, b, _) in &bn.cabin.links {
            let (Some(pa), Some(pb)) = (pts.get(a.max(0) as usize), pts.get(b.max(0) as usize))
            else {
                continue;
            };
            let (a2, b2) = (pa.truncate(), pb.truncate());
            let ab = b2 - a2;
            let t = if ab.length_squared() > 1e-6 {
                ((want - a2).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let z = pa.z + (pb.z - pa.z) * t;
            let d = (want - (a2 + ab * t)).length() + (local.z - z).abs();
            if best.is_none_or(|x| d < x.0) {
                best = Some((d, a2 + ab * t, z));
            }
        }
        if best.is_none() {
            for pt in pts {
                let d = (want - pt.truncate()).length();
                if best.is_none_or(|x| d < x.0) {
                    best = Some((d, pt.truncate(), pt.z));
                }
            }
        }
        let (d, q, z) = best?;
        let xy = if d > WIDTH {
            q + (want - q) / d * WIDTH
        } else {
            want
        };
        // no nearer to a seat or the driver's place than 0.38 m (walking away is let be)
        let from = local.truncate();
        let solid = bn
            .cabin
            .seats
            .iter()
            .filter(|s| s.seated)
            .map(|s| s.pos.truncate())
            .chain(
                bn.cabin
                    .data
                    .driver_positions
                    .iter()
                    .map(|d| glam::Vec2::new(d.pos[0], d.pos[1])),
            );
        for c in solid {
            let dn = (xy - c).length();
            if dn < 0.38 && dn < (from - c).length() {
                return Some((local, bn.world(local)));
            }
        }
        let l = Vec3::new(xy.x, xy.y, z);
        Some((l, bn.world(l)))
    }

    /// (threshold, outside in the world, side +1 right, open)
    pub fn cabin_doors(&self, bus: BusId) -> Vec<(Vec3, DVec3, f32, bool)> {
        let Some(bn) = self.bus_now(bus) else {
            return Vec::new();
        };
        let (eo, xo) = bn
            .walk_open
            .as_ref()
            .map_or((&bn.entry_open, &bn.exit_open), |w| (&w.0, &w.1));
        let open = |o: &Vec<bool>, k: usize| o.get(k).copied().unwrap_or(false);
        let entries = bn
            .cabin
            .entries
            .iter()
            .enumerate()
            .map(|(k, d)| (d, open(eo, k)));
        let exits = bn
            .cabin
            .exits
            .iter()
            .enumerate()
            .map(|(k, d)| (d, open(xo, k)));
        entries
            .chain(exits)
            .map(|(d, open)| (d.inside, bn.world(d.outside), d.side, open))
            .collect()
    }

    pub fn bus_ids_near(&self, at: DVec3, r: f64) -> Vec<BusId> {
        let mut v: Vec<(BusId, f64)> = self
            .last_buses
            .iter()
            .map(|b| (b.id, (b.pos - at).truncate().length()))
            .filter(|x| x.1 < r)
            .collect();
        v.sort_by(|a, b| {
            (a.0 != BusId::Player)
                .cmp(&(b.0 != BusId::Player))
                .then(a.1.total_cmp(&b.1))
        });
        v.into_iter().map(|x| x.0).collect()
    }

    pub fn cabin_world(&self, bus: BusId, local: Vec3) -> Option<(DVec3, f64)> {
        let bn = self.bus_now(bus)?;
        Some((bn.world(local), bn.heading))
    }

    pub fn seat_nearest(&self, bus: BusId, at: DVec3, reach: f64) -> Option<usize> {
        let bn = self.bus_now(bus)?;
        self.free_seats(bn)
            .map(|(k, s)| (k, (bn.world(s.floor) - at).truncate().length()))
            .filter(|(_, d)| *d < reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    }

    /// On the seat's own deck: in a double-decker the nearest in plan can be above or below it.
    pub fn seat_stand(&self, bus: BusId, seat: usize) -> Option<Vec3> {
        let bn = self.bus_now(bus)?;
        let f = bn.cabin.seats.get(seat)?.floor;
        nearest_on_deck(&bn.cabin.points, f)
    }

    pub fn vehicle_cabin_world(&mut self, v: &VehicleInstance, local: Vec3) -> Option<DVec3> {
        let cabin = self.cabin_for(v)?;
        Some(train_point(
            v.position,
            &v.body_rotation(),
            &part_frames(v, &cabin),
            local,
        ))
    }

    /// Half a metre under the driver's hip: an upper deck over the cab is as near in plan.
    pub fn driver_stand(&mut self, v: &VehicleInstance) -> Option<Vec3> {
        let cabin = self.cabin_for(v)?;
        let seat = cabin
            .data
            .driver_positions
            .first()
            .map_or(Vec3::new(-0.8, 4.5, 1.0), |d| Vec3::from(d.pos));
        nearest_on_deck(&cabin.points, seat - Vec3::Z * 0.5)
    }

    pub fn people_in(&self, bus: BusId) -> usize {
        self.people
            .iter()
            .filter(|p| {
                matches!(p.place, Place::Bus(b, _) if b == bus) || p.state.bus() == Some(bus)
            })
            .count()
    }

    pub fn bus_center(&self, bus: BusId) -> Option<DVec3> {
        self.bus_now(bus).map(|b| b.pos)
    }

    pub fn bus_here(&self, bus: BusId) -> bool {
        self.bus_now(bus).is_some()
    }

    pub(super) fn animate_avatar(&mut self, i: usize, dt: f32, f: &Frame) {
        let id = self.people[i].id;
        let Some(cmd) = self
            .avatars
            .ids
            .iter()
            .find(|(_, v)| **v == id)
            .and_then(|(k, _)| self.avatars.cmds.get(k))
            .copied()
        else {
            return;
        };
        let bus = |b: BusId| f.bus(Some(b));
        let seated = cmd
            .seat
            .and_then(|(b, k)| Some((b, bus(b)?.cabin.seats.get(k)?.clone(), bus(b)?)));
        let seat_height = self.people[i].ty.def.seat_height;
        let p = &mut self.people[i];
        let aboard = |p: &mut Person, b: BusId, l: Vec3, bn: &BusNow| {
            p.place = Place::Bus(b, l);
            p.position = bn.world(l);
            p.tilt = bn.tilt_at(l);
            p.interior = bn.interior;
        };
        let speed = cmd.vel.length() as f32;
        let input = match (seated, cmd.aboard.and_then(|(b, l)| Some((b, l, bus(b)?)))) {
            (Some((b, s, bn)), _) => {
                // on the seat in its bus's frame, the feet the seat height under the seat point
                let l = if s.seated {
                    s.pos - Vec3::Z * seat_height
                } else {
                    s.pos
                };
                aboard(p, b, l, bn);
                p.lheading = s.rot as f64;
                p.heading = bn.heading_at(l) + p.lheading;
                p.vel = DVec2::ZERO;
                p.activity = if s.seated {
                    Activity::Sit
                } else {
                    Activity::Stand
                };
                AnimInput {
                    kind: if s.seated { 2 } else { 0 },
                    seat_height: s.height,
                    room_height: OUTSIDE_ROOM,
                    dt_ms: dt * 1000.0,
                    ..Default::default()
                }
            }
            (None, Some((b, l, bn))) => {
                aboard(p, b, l, bn);
                p.lheading = wrap_heading(cmd.heading - bn.heading_at(l));
                p.heading = cmd.heading;
                p.vel = cmd.vel;
                p.activity = if speed > 0.05 {
                    Activity::Walk
                } else {
                    Activity::Stand
                };
                walk_input(speed, dt)
            }
            (None, None) => {
                p.place = Place::Ground;
                p.tilt = Mat4::IDENTITY;
                p.interior = 0.0;
                let ground = cmd.floor.unwrap_or_else(|| {
                    f.world
                        .walk_height(cmd.pos.x, cmd.pos.y)
                        .unwrap_or(cmd.pos.z)
                });
                let z = if cmd.floor.is_some() {
                    ground
                } else {
                    cmd.pos.z.max(ground)
                };
                p.position = DVec3::new(cmd.pos.x, cmd.pos.y, z + cmd.lift.max(0.0));
                p.heading = cmd.heading;
                p.vel = cmd.vel;
                p.activity = if speed > 0.05 {
                    Activity::Walk
                } else {
                    Activity::Stand
                };
                walk_input(speed, dt)
            }
        };
        p.anim.advance(&p.ty.omsi, &input);
    }
}

fn nearest_on_deck(points: &[Vec3], at: Vec3) -> Option<Vec3> {
    let d = |a: &Vec3| (a.truncate() - at.truncate()).length() + (a.z - at.z).abs() * 3.0;
    points.iter().copied().min_by(|a, b| d(a).total_cmp(&d(b)))
}
