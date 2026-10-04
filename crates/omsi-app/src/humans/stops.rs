use super::*;

const STOP_RANGE: f64 = 450.0;

#[derive(Debug, Clone)]
pub(super) struct WaitSpot {
    pub(super) pos: DVec3,
    pub(super) face: f64,
    pub(super) height: f32,
}

pub(super) struct PaxStop {
    pub(super) name: String,
    pub(super) alias: String,
    pub(super) pos: DVec3,
    pub(super) heading: f64,
    pub(super) gather: DVec3,
    pub(super) spots: Vec<WaitSpot>,
    pub(super) taken: Vec<bool>,
    pub(super) enter_max: f32,
    pub(super) enter_min: f32,
    pub(super) length: f32,
    pub(super) lane: Option<(usize, f32)>,
    pub(super) near: bool,
    pub(super) clock_ms: f32,
    pub(super) factor: f32,
    pub(super) buses: Vec<(BusId, bool)>,
    pub(super) dests: Vec<(String, f32)>,
    pub(super) lines: Vec<(String, HashSet<String>)>,
}

impl PaxStop {
    pub(super) fn is_named(&self, name: &str) -> bool {
        let name = name.trim();
        name == self.name.trim() || (!self.alias.is_empty() && name == self.alias.trim())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct BusAtStops {
    pub(super) next: Option<i64>,
    pub(super) near: Vec<i64>,
    pub(super) all_exit: bool,
}

impl Humans {
    /// A stop nobody waits at is set up again with what its tiles hold now.
    pub(super) fn tiles_changed(&mut self, world: &World) {
        let present: HashSet<i64> = world.bus_stops.lock().iter().map(|s| s.0).collect();
        let bound = |st: &State| match st {
            State::Pax(p) if p.inside.is_none() => p.stop,
            _ => None,
        };
        let used: HashSet<i64> = self.people.iter().filter_map(|p| bound(&p.state)).collect();
        let gone: Vec<i64> = self
            .stops
            .keys()
            .copied()
            .filter(|id| !present.contains(id))
            .collect();
        for i in (0..self.people.len()).rev() {
            let p = &self.people[i];
            let lost_stop = bound(&p.state).is_some_and(|s| gone.contains(&s));
            let lost_ground = p.place == Place::Ground
                && !p.avatar
                && !world.has_ground(p.position.x, p.position.y);
            if lost_stop || lost_ground {
                self.remove_person(i);
            }
        }
        self.stops
            .retain(|id, _| present.contains(id) && used.contains(id));
    }

    pub fn populate(&mut self, world: &World, center: DVec3) {
        if self.avatar_only {
            return;
        }
        // onto the pavements that came after them
        for p in self.people.iter_mut().filter(|p| p.place == Place::Ground) {
            if let Some(z) = world.walk_height_near(p.position.x, p.position.y, p.position.z) {
                let d = z - p.position.z;
                let still = p.vel.length() < 0.05;
                if d.abs() < 3.0 && (d > 0.02 || (still && d.abs() > 0.02)) {
                    p.position.z = z;
                }
            }
        }
        self.populate_with(world, None, center);
    }

    pub(super) fn populate_with(&mut self, world: &World, net: Option<&Network>, center: DVec3) {
        self.center = center;
        let list: Vec<(i64, DVec3, f64, String)> = world
            .bus_stops
            .lock()
            .iter()
            .filter(|s| (s.1 - center).length() < STOP_RANGE + 100.0)
            .map(|s| (s.0, s.1, s.2, s.3.clone()))
            .collect();
        for (id, pos, rot, name) in list {
            if world.walk_height(pos.x, pos.y).is_some() && !self.stops.contains_key(&id) {
                let st = self.build_pax_stop(world, net, id, pos, rot, &name);
                self.stops.insert(id, st);
            }
        }
    }

    /// The waiting places: the `[passpos]` of every object within 10 m to the platform's side and
    /// from 10 m behind to the stop's length ahead (sub_620c0c).
    fn build_pax_stop(
        &mut self,
        world: &World,
        net: Option<&Network>,
        id: i64,
        pos: DVec3,
        heading: f64,
        name: &str,
    ) -> PaxStop {
        let length = world.stop_length(id);
        let side = world.stop_side(id).round().clamp(0.0, 255.0) as u8;
        let left = LEFT_HAND.load(std::sync::atomic::Ordering::Relaxed);
        let xmax = if (side == 1) != left { 0.0 } else { 10.0 };
        let xmin = if (side == 0) != left { 0.0 } else { -10.0 };
        let h = heading.to_radians();
        let objects = world.object_positions.lock();
        let spots: Vec<WaitSpot> = world
            .waiting_places
            .lock()
            .iter()
            .filter(|(obj, ..)| {
                let Some((opos, _)) = objects.get(obj) else {
                    return false;
                };
                let v = pos - *opos;
                // a long bus station stop reaches past 40 m
                if v.length() > 40.0_f64.max(length as f64 + 15.0) {
                    return false;
                }
                let lat = h.cos() * v.x - h.sin() * v.y;
                let along = h.cos() * v.y + h.sin() * v.x;
                -along < 10.0 && -along > -(length as f64).max(10.0) && -lat < xmax && -lat > xmin
            })
            .map(|&(_, pos, face, height)| WaitSpot { pos, face, height })
            .collect();
        drop(objects);
        let x = if (side == 1) == left { 1.0 } else { -1.0 };
        let (fwd, right) = (DVec2::new(h.sin(), h.cos()), DVec2::new(h.cos(), -h.sin()));
        let g = pos.truncate() + right * x + fwd * 1.0;
        let lane = net
            .zip(self.ped.as_ref())
            .and_then(|(n, pn)| pn.nearest(n, pos, 12.0))
            .map(|(l, s, _)| (l, s));
        let (enter_max, enter_min) = world.stop_enter(id);
        // the stops the trips from here go on to, as likely as people get off there
        let lines: Vec<(String, HashSet<String>)> = self
            .stop_targets
            .as_ref()
            .and_then(|m| m.get(&id))
            .cloned()
            .unwrap_or_default();
        let weights: Vec<f32> = lines
            .iter()
            .map(|(n, _)| {
                let stops = world.bus_stops.lock();
                stops
                    .iter()
                    .find(|s| s.3.trim() == n.trim())
                    .map_or(0.5, |s| world.stop_exit_weight(s.0))
            })
            .collect();
        let total: f32 = weights.iter().sum();
        let dests = if total > 0.0 {
            lines
                .iter()
                .zip(&weights)
                .map(|((n, _), w)| (n.clone(), w / total))
                .collect()
        } else {
            Vec::new()
        };
        log::debug!(
            "stop {id} '{name}' at {pos:.1}, heading {heading:.0}: {} waiting places, {} destinations",
            spots.len(),
            lines.len()
        );
        // its id when the timetable does not know it, as the targets then name it
        let alias = match &self.stop_names {
            Some(n) => n.get(&id).cloned().unwrap_or_else(|| id.to_string()),
            None => String::new(),
        };
        PaxStop {
            name: name.to_string(),
            alias,
            pos,
            heading,
            gather: DVec3::new(g.x, g.y, pos.z),
            taken: vec![false; spots.len()],
            spots,
            enter_max,
            enter_min,
            length,
            lane,
            near: false,
            clock_ms: 0.0,
            factor: 1.0,
            buses: Vec::new(),
            dests,
            lines,
        }
    }

    /// sub_61bf94: a stop coming into range fills at once, one in range gets one person every
    /// 10..15 s up to its pass_enter mean times its factor and the density.
    pub(super) fn stops_tick(
        &mut self,
        dt: f32,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
    ) {
        if self.lan.mirror || self.avatar_only {
            return;
        }
        // the people handed over to another player's bus count until it has left their stop
        let (stops, remote) = (&self.stops, &self.lan.remote_now);
        self.lan.handed.retain(|(stop, bus)| {
            stops.get(stop).is_some_and(|s| {
                remote
                    .iter()
                    .any(|b| b.id == BusId::Ai(*bus) && (b.pos - s.pos).length() < 60.0)
            })
        });
        let mut ids: Vec<i64> = self.stops.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let pos = self.stops[&id].pos;
            let near = (pos - self.center).length() < STOP_RANGE
                || self
                    .lan_centers
                    .iter()
                    .any(|c| (pos - *c).length() < STOP_RANGE);
            let s = self.stops.get_mut(&id).unwrap();
            let changed = s.near != near;
            s.near = near;
            if !near {
                if changed {
                    for i in (0..self.people.len()).rev() {
                        let here = matches!(&self.people[i].state, State::Pax(p) if p.stop == Some(id)
                            && p.inside.is_none()
                            && matches!(p.task, Task::WaitingForBus | Task::WalkingToBusstop));
                        if here {
                            self.remove_person(i);
                        }
                    }
                    self.stops.get_mut(&id).unwrap().taken.fill(false);
                }
                continue;
            }
            if changed {
                let r = self.rand_f() as f32;
                let s = self.stops.get_mut(&id).unwrap();
                let mean = (s.enter_max + s.enter_min) / 2.0;
                let k = if s.enter_max == 0.0 {
                    0.0
                } else {
                    (s.enter_max - s.enter_min)
                        / (if mean == 0.0 { s.enter_max } else { mean } * 2.0)
                };
                s.factor = (r * 2.0 - 1.0) * k + 1.0;
            }
            let mut count = self
                .people
                .iter()
                .filter(|p| matches!(&p.state, State::Pax(x) if x.stop == Some(id)))
                .count()
                + self.lan.handed.iter().filter(|h| h.0 == id).count();
            let s = self.stops.get_mut(&id).unwrap();
            let mean = (s.enter_max + s.enter_min) / 2.0;
            let want = ((self.density.max(0.0) * mean * s.factor).round().max(0.0) as usize)
                .min(s.spots.len());
            s.clock_ms += dt * 1000.0;
            if !changed && self.stops[&id].clock_ms <= self.rand_f() as f32 * 5000.0 + 10000.0 {
                continue;
            }
            while count < want && self.spawn_waiting(world, renderer, scene, id).is_some() {
                count += 1;
                if !changed {
                    break;
                }
            }
            if count >= want {
                self.stops.get_mut(&id).unwrap().clock_ms = 0.0;
            }
        }
    }

    pub(super) fn draw_dest(&mut self, id: i64) -> (Option<String>, Option<usize>) {
        let mut r = self.rand_f() as f32;
        let Some(stop) = self.stops.get(&id) else {
            return (None, None);
        };
        let mut dest: Option<String> = None;
        for (n, w) in &stop.dests {
            if r <= 0.0 {
                break;
            }
            r -= w;
            if r <= 0.0 {
                dest = Some(n.clone());
            }
        }
        let line = dest
            .as_ref()
            .and_then(|d| stop.lines.iter().position(|(n, _)| n.trim() == d.trim()));
        (dest, line)
    }

    fn spawn_waiting(
        &mut self,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        id: i64,
    ) -> Option<usize> {
        let k = self.take_spot(id)?;
        let sp = self.stops[&id].spots[k].clone();
        let (dest, line) = self.draw_dest(id);
        let mut pax = Pax::new(self.walk_pace() as f32);
        pax.stop = Some(id);
        pax.spot = Some(k);
        pax.pos = sp.pos;
        pax.yaw = sp.face.to_radians();
        pax.dest = dest;
        pax.line = line;
        let Some(i) = self.spawn(
            world,
            renderer,
            scene,
            sp.pos,
            sp.face,
            State::Pax(Box::new(pax)),
        ) else {
            self.free_spot(id, k);
            return None;
        };
        self.set_task(
            i,
            Task::WalkingToBusstop,
            &Frame::new(world, None, Vec::new(), HashMap::new()),
        );
        log::debug!("{} waits at stop {id} place {k}", self.people[i].label());
        Some(i)
    }

    pub(super) fn register_buses(
        &mut self,
        buses: &[BusNow],
        dt: f32,
    ) -> HashMap<BusId, BusAtStops> {
        let mut out: HashMap<BusId, BusAtStops> = HashMap::new();
        for s in self.stops.values_mut() {
            s.buses.clear();
        }
        let mut ids: Vec<i64> = self.stops.keys().copied().collect();
        ids.sort_unstable();
        for bn in buses {
            *self.odometer.entry(bn.id).or_insert(0.0) += bn.speed.abs() * dt as f64 / 1000.0;
            let mut reg = BusAtStops::default();
            for id in &ids {
                let s = &self.stops[id];
                let d = bn.pos - s.pos;
                if !(d.length() < 60.0) {
                    continue;
                }
                let sh = s.heading.to_radians();
                let (s_fwd, s_right) = (
                    DVec2::new(sh.sin(), sh.cos()),
                    DVec2::new(sh.cos(), -sh.sin()),
                );
                let same_way = bn.fwd().dot(s_fwd) > 0.0;
                reg.near.push(*id);
                if same_way {
                    reg.next = Some(*id);
                }
                // a bus not in service, or at its own terminus, empties and takes nobody (0x61f3e3)
                if bn.terminus.as_ref().is_none_or(|t| s.is_named(t)) {
                    reg.all_exit = true;
                    continue;
                }
                if same_way {
                    let (lateral, along) = (d.truncate().dot(s_right), d.truncate().dot(s_fwd));
                    let in_box =
                        lateral.abs() < 2.0 && along.abs() < (s.length as f64 - 5.0).max(0.0);
                    self.stops.get_mut(id).unwrap().buses.push((bn.id, in_box));
                }
            }
            out.insert(bn.id, reg);
        }
        out
    }

    pub(super) fn bus_for(&self, i: usize, stop: i64, f: &Frame) -> Option<BusId> {
        let s = self.stops.get(&stop)?;
        let Some((_, termini)) = self.pax(i)?.line.and_then(|k| s.lines.get(k)) else {
            return s.buses.first().map(|b| b.0);
        };
        s.buses
            .iter()
            .filter_map(|(id, _)| f.bus(Some(*id)))
            .filter(|bn| {
                !bn.cabin.entries.is_empty()
                    && bn
                        .terminus
                        .as_ref()
                        .is_some_and(|t| termini.contains(t.trim()))
            })
            .map(|bn| ((bn.pos - self.people[i].position).length(), bn.id))
            .fold(None, |best: Option<(f64, BusId)>, c| {
                if best.is_none_or(|b| c.0 < b.0) {
                    Some(c)
                } else {
                    best
                }
            })
            .map(|b| b.1)
    }

    pub(super) fn in_stop_box(&self, stop: i64, bus: BusId) -> bool {
        self.stops
            .get(&stop)
            .is_some_and(|s| s.buses.iter().any(|b| b.0 == bus && b.1))
    }

    pub(super) fn listed_at(&self, stop: i64, bus: BusId) -> bool {
        self.stops
            .get(&stop)
            .is_some_and(|s| s.buses.iter().any(|b| b.0 == bus))
    }

    pub(super) fn free_spot(&mut self, stop: i64, k: usize) {
        if let Some(t) = self.stops.get_mut(&stop).and_then(|s| s.taken.get_mut(k)) {
            *t = false;
        }
    }

    pub(super) fn take_spot(&mut self, stop: i64) -> Option<usize> {
        let free: Vec<usize> = self
            .stops
            .get(&stop)?
            .taken
            .iter()
            .enumerate()
            .filter(|(_, t)| !**t)
            .map(|(k, _)| k)
            .collect();
        if free.is_empty() {
            return None;
        }
        let k = free[(self.rand() as usize) % free.len()];
        self.stops.get_mut(&stop).unwrap().taken[k] = true;
        Some(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_answers_to_its_label_and_its_timetable_name() {
        let stop = |alias: &str| PaxStop {
            name: "Königsrath, Bf. Ausstieg".into(),
            alias: alias.into(),
            pos: DVec3::ZERO,
            heading: 0.0,
            gather: DVec3::ZERO,
            spots: Vec::new(),
            taken: Vec::new(),
            enter_max: 1.0,
            enter_min: 0.0,
            length: 30.0,
            lane: None,
            near: false,
            clock_ms: 0.0,
            factor: 1.0,
            buses: Vec::new(),
            dests: Vec::new(),
            lines: Vec::new(),
        };
        let s = stop("Koenigsrath Bf Ausstieg");
        assert!(s.is_named("Königsrath, Bf. Ausstieg "));
        assert!(
            s.is_named("Koenigsrath Bf Ausstieg"),
            "the timetable's spelling"
        );
        assert!(!s.is_named("Königsrath, Bf. Pause"));
        assert!(stop("4711").is_named("4711"));
        assert!(!stop("").is_named(""), "no timetable name: no empty match");
    }
}
