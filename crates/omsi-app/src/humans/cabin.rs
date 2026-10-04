use super::*;

const DOOR_OUT: f32 = 0.5;
const SEAT_FRONT: f32 = 0.34;

#[derive(Debug, Clone)]
pub(super) struct Door {
    pub(super) inside: Vec3,
    pub(super) point: Option<usize>,
    pub(super) outside: Vec3,
    pub(super) side: f32,
    pub(super) sells: bool,
    pub(super) button: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Seat {
    pub(super) pos: Vec3,
    pub(super) floor: Vec3,
    pub(super) rot: f32,
    pub(super) seated: bool,
    pub(super) height: f32,
    pub(super) omsi_seat: usize,
}

/// The sections of an articulated bus are joined in the front section's frame, unbent.
pub(super) struct Cabin {
    pub(super) data: PassengerCabin,
    pub(super) points: Vec<Vec3>,
    pub(super) links: Vec<(i32, i32, bool)>,
    pub(super) link_pack: Vec<Option<usize>>,
    pub(super) link_room: Vec<f32>,
    pub(super) step_packs: Vec<Arc<[String]>>,
    pub(super) entries: Vec<Door>,
    pub(super) exits: Vec<Door>,
    pub(super) seats: Vec<Seat>,
    pub(super) parts: Vec<CabinPart>,
    pub(super) routes: Vec<Vec<RouteLink>>,
    pub(super) stamper: Option<(Option<usize>, Vec3)>,
    pub(super) sale: Option<(Option<usize>, Vec3)>,
    pub(super) money: Option<(Vec3, [f32; 2])>,
    pub(super) change_point: Option<Vec3>,
}

pub(super) fn seat_numbers(seats: &[Seat], sitting: impl Iterator<Item = usize>) -> Vec<u32> {
    let n = seats.iter().map(|s| s.omsi_seat + 1).max().unwrap_or(0);
    let mut out = vec![0u32; n];
    for k in sitting {
        if let Some(c) = seats.get(k).and_then(|s| out.get_mut(s.omsi_seat)) {
            *c += 1;
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CabinPart {
    pub(super) offset: Vec3,
    pub(super) joint_y: f32,
}

pub(super) type TrainPart<'a> = (&'a omsi_vehicle::Vehicle, Vec3, f32);

pub(super) fn train_parts(v: &VehicleInstance) -> Vec<TrainPart<'_>> {
    let mut out: Vec<TrainPart<'_>> = vec![(&v.ty.def, Vec3::ZERO, f32::INFINITY)];
    let mut offset = Vec3::ZERO;
    for t in v.trailers.iter().take_while(|t| !t.reversed) {
        let (back, front) = t.couplings();
        let joint_y = offset.y + back.y;
        offset += back - front;
        out.push((&t.ty.def, offset, joint_y));
    }
    out
}

impl Cabin {
    pub(super) fn load_train(parts: &[TrainPart<'_>]) -> Option<Cabin> {
        let load_cabin = |def: &omsi_vehicle::Vehicle| -> Option<PassengerCabin> {
            let rel = def.passenger_cabin.as_ref()?;
            PassengerCabin::load(&omsi_cfg::resolve_path(def.dir(), rel))
                .map_err(|e| log::warn!("{e}"))
                .ok()
        };
        let load_paths = |def: &omsi_vehicle::Vehicle| {
            let rel = def.paths.as_ref()?;
            omsi_vehicle::VehiclePaths::load(&omsi_cfg::resolve_path(def.dir(), rel))
                .map_err(|e| log::warn!("{e}"))
                .ok()
        };
        let data = load_cabin(parts.first()?.0)?;
        let mut points: Vec<Vec3> = Vec::new();
        let mut links: Vec<(i32, i32, bool)> = Vec::new();
        let mut link_pack: Vec<Option<usize>> = Vec::new();
        let mut link_room: Vec<f32> = Vec::new();
        let mut step_packs: Vec<Arc<[String]>> = Vec::new();
        // (path point or -1, sells tickets, {withbutton}, half width of the section)
        let mut entry_points: Vec<(i32, bool, bool, f32)> = Vec::new();
        let mut exit_points: Vec<(i32, f32)> = Vec::new();
        let mut places: Vec<(omsi_vehicle::cabin::PassPos, Vec3, usize)> = Vec::new();
        let mut seat_base = 0usize;
        let mut cabin_parts: Vec<CabinPart> = Vec::new();
        let mut rear_link: Option<usize> = None;
        for (k, &(def, offset, joint_y)) in parts.iter().enumerate() {
            let Some(cab) = (if k == 0 {
                Some(data.clone())
            } else {
                load_cabin(def)
            }) else {
                break;
            };
            let paths = load_paths(def).unwrap_or_default();
            let own: Vec<Vec3> = paths
                .points
                .iter()
                .map(|q| Vec3::from(q.pos) + offset)
                .collect();
            let base = points.len();
            let valid = |i: i32| (i >= 0 && (i as usize) < own.len()).then_some(base + i as usize);
            let end = |front: bool| {
                (0..own.len())
                    .filter(|i| own[*i].x.abs() < 0.6)
                    .max_by(|a, b| {
                        if front {
                            own[*a].y.total_cmp(&own[*b].y)
                        } else {
                            own[*b].y.total_cmp(&own[*a].y)
                        }
                    })
                    .map(|i| base + i)
            };
            if k > 0 {
                let front = cab.link_to_next_veh.and_then(valid).or_else(|| end(true));
                let (Some(a), Some(b)) = (rear_link, front) else {
                    break;
                };
                links.push((a as i32, b as i32, false));
                link_pack.push(None);
                link_room.push(2.0);
            }
            points.extend(own.iter().copied());
            links.extend(
                paths
                    .links
                    .iter()
                    .map(|(a, b, o)| (a + base as i32, b + base as i32, *o)),
            );
            let n_packs = paths.step_sound_packs.len();
            let pack_base = step_packs.len();
            link_pack.extend((0..paths.links.len()).map(|i| {
                let n = paths.link_step_sound.get(i).copied().unwrap_or(-1);
                (n >= 0 && (n as usize) < n_packs).then(|| pack_base + n as usize)
            }));
            link_room.extend(
                (0..paths.links.len())
                    .map(|i| paths.link_room_height.get(i).copied().unwrap_or(2.0)),
            );
            step_packs.extend(paths.step_sound_packs.into_iter().map(Arc::from));
            rear_link = cab.link_to_prev_veh.and_then(valid).or_else(|| end(false));
            let half = def
                .bounding_box
                .map(|b| b[0] * 0.5)
                .unwrap_or_else(|| own.iter().map(|p| p.x.abs()).fold(1.2, f32::max));
            let shift = |i: i32| valid(i).map(|m| m as i32).unwrap_or(-1);
            entry_points.extend(
                cab.entries
                    .iter()
                    .map(|e| (shift(e.path_point), !e.no_ticket_sale, e.with_button, half)),
            );
            exit_points.extend(cab.exits.iter().map(|e| (shift(*e), half)));
            places.extend(
                cab.pass_positions
                    .iter()
                    .map(|p| (p.clone(), offset, seat_base + p.file_index)),
            );
            seat_base += cab.pass_positions.len() + cab.driver_positions.len();
            cabin_parts.push(CabinPart { offset, joint_y });
        }
        // a door on the aisle (or without a point) opens to the kerb
        let kerb = if LEFT_HAND.load(std::sync::atomic::Ordering::Relaxed) {
            -1.0f32
        } else {
            1.0
        };
        let door = |pp: i32, sells: bool, button: bool, half_width: f32| -> Door {
            let point = (pp >= 0 && (pp as usize) < points.len()).then_some(pp as usize);
            let inside =
                point
                    .map(|i| points[i])
                    .unwrap_or(Vec3::new(kerb * (half_width - 0.1), 4.0, 0.4));
            let side = if inside.x.abs() < 0.6 {
                kerb
            } else {
                inside.x.signum()
            };
            Door {
                inside,
                point,
                outside: Vec3::new(side * (half_width + DOOR_OUT), inside.y, 0.0),
                side,
                sells,
                button,
            }
        };
        let entries = entry_points
            .iter()
            .map(|&(pp, sells, button, half)| door(pp, sells, button, half))
            .collect();
        let exits = exit_points
            .iter()
            .map(|&(pp, half)| door(pp, false, false, half))
            .collect();
        let seats = places
            .iter()
            .map(|(p, offset, omsi_seat)| {
                let pos = Vec3::from(p.pos) + *offset;
                let seated = p.height > 0.01;
                let r = p.rot.to_radians();
                let floor = if seated {
                    Vec3::new(
                        pos.x + r.sin() * SEAT_FRONT,
                        pos.y + r.cos() * SEAT_FRONT,
                        pos.z - p.height,
                    )
                } else {
                    pos
                };
                Seat {
                    pos,
                    floor,
                    rot: p.rot,
                    seated,
                    height: p.height,
                    omsi_seat: *omsi_seat,
                }
            })
            .collect();
        let point_of = |i: i32| usize::try_from(i).ok().filter(|i| *i < points.len());
        Some(Cabin {
            routes: build_routes(points.len(), &links),
            stamper: data
                .stampers
                .last()
                .map(|st| (point_of(st.path_point), Vec3::from(st.pos))),
            sale: data
                .ticket_sales
                .last()
                .map(|st| (point_of(st.path_point), Vec3::from(st.pos))),
            money: data.money_points.last().map(|m| (Vec3::from(m.pos), m.var)),
            change_point: data.change_points.last().map(|m| Vec3::from(m.pos)),
            data,
            points,
            links,
            link_pack,
            link_room,
            step_packs,
            entries,
            exits,
            seats,
            parts: cabin_parts,
        })
    }

    pub(super) fn all_points(&self) -> Vec<Option<usize>> {
        (0..self.points.len()).map(Some).collect()
    }

    /// sub_72506c, height weighed by 5. Nothing found: the first of the list, or the search again
    /// without `avoid`.
    pub(super) fn omsi_nearest(
        &self,
        p: Vec3,
        list: &[Option<usize>],
        avoid: bool,
        level: bool,
        flags: Option<&[(bool, bool)]>,
        open: Option<&[bool]>,
    ) -> Option<usize> {
        let mut best = 1e12f32;
        let mut found: Option<usize> = None;
        for (k, pt) in list.iter().enumerate() {
            let Some((pt, q)) = pt.and_then(|pt| self.points.get(pt).map(|q| (pt, *q))) else {
                continue;
            };
            if level && !(q.z <= p.z && p.z <= q.z + 2.0) {
                continue;
            }
            let shut = open.is_some_and(|o| o.len() >= list.len() && !o[k]);
            if shut && !flags.is_some_and(|f| f.get(k).is_some_and(|f| f.1)) {
                continue;
            }
            if avoid && flags.is_some_and(|f| f.len() >= list.len() && f[k].0) {
                continue;
            }
            let d = p - q;
            let d = Vec3::new(d.x, d.y, d.z * 5.0).length();
            if d < best {
                best = d;
                found = Some(pt);
            }
        }
        match found {
            None if avoid && flags.is_some() => {
                self.omsi_nearest(p, list, false, level, flags, open)
            }
            None => list.first().copied().flatten(),
            found => found,
        }
    }

    pub(super) fn route_next(&self, from: usize, to: usize) -> Option<(usize, usize)> {
        self.routes
            .get(from)?
            .iter()
            .find(|l| l.reach.contains(&to))
            .map(|l| (l.to, l.link))
    }

    pub(super) fn entry_points(&self) -> Vec<Option<usize>> {
        self.entries.iter().map(|e| e.point).collect()
    }

    pub(super) fn exit_points(&self) -> Vec<Option<usize>> {
        self.exits.iter().map(|e| e.point).collect()
    }

    pub(super) fn entry_flags(&self) -> Vec<(bool, bool)> {
        self.entries.iter().map(|e| (!e.sells, e.button)).collect()
    }
}

#[derive(Debug, Clone)]
pub(super) struct RouteLink {
    pub(super) to: usize,
    pub(super) reach: Vec<usize>,
    pub(super) link: usize,
    walk_back: bool,
}

/// sub_72410c: a depth-first walk from every point; a one-way link is walked back only from
/// its end.
pub(super) fn build_routes(n: usize, links: &[(i32, i32, bool)]) -> Vec<Vec<RouteLink>> {
    let mut adj: Vec<Vec<RouteLink>> = vec![Vec::new(); n];
    for (k, &(a, b, oneway)) in links.iter().enumerate() {
        if a < 0 || b < 0 || a as usize >= n || b as usize >= n {
            continue;
        }
        let (a, b) = (a as usize, b as usize);
        adj[a].push(RouteLink {
            to: b,
            reach: vec![b],
            link: k,
            walk_back: !oneway,
        });
        adj[b].push(RouteLink {
            to: a,
            reach: vec![a],
            link: k,
            walk_back: true,
        });
    }
    fn visit(adj: &mut Vec<Vec<RouteLink>>, p: usize, from: Option<usize>, stack: &mut Vec<usize>) {
        if let Some(k) = from.and_then(|q| adj[p].iter().position(|l| l.to == q)) {
            for &s in stack.iter() {
                if !adj[p][k].reach.contains(&s) {
                    adj[p][k].reach.push(s);
                }
            }
        }
        if !stack.contains(&p) {
            stack.push(p);
        }
        for k in 0..adj[p].len() {
            let to = adj[p][k].to;
            if !stack.contains(&to) && adj[p][k].walk_back {
                visit(adj, to, Some(p), stack);
            }
        }
    }
    for root in 0..n {
        visit(&mut adj, root, None, &mut Vec::new());
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(cabin: &Cabin, mut at: usize, to: usize) -> Vec<Vec3> {
        let mut route = vec![cabin.points[at]];
        while at != to {
            at = cabin.route_next(at, to).expect("a way on").0;
            route.push(cabin.points[at]);
            assert!(route.len() < 60, "{route:?}");
        }
        route
    }

    fn stock_bus(file: &str) -> Option<omsi_vehicle::Vehicle> {
        let root = omsi_cfg::env::var_os("OMSI_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("../../../OMSI 2 Original"));
        let bus = root.join("Vehicles").join(file);
        if !bus.exists() {
            eprintln!("skipped: no {}", bus.display());
            return None;
        }
        Some(omsi_vehicle::Vehicle::load(&bus).expect("bus"))
    }

    #[test]
    fn seats_counted_by_the_scripts_numbers() {
        let seat = |omsi_seat: usize| Seat {
            pos: Vec3::ZERO,
            floor: Vec3::ZERO,
            rot: 0.0,
            seated: true,
            height: 0.45,
            omsi_seat,
        };
        // the driver's place is seat 0, a second section's numbers follow the first's
        let seats = [seat(1), seat(2), seat(4), seat(6)];
        assert_eq!(
            seat_numbers(&seats, [0, 2, 2, 3].into_iter()),
            [0, 1, 0, 0, 2, 0, 1]
        );
        assert!(seat_numbers(&[], [0].into_iter()).is_empty());
    }

    #[test]
    fn articulated_cabins_are_joined_through_the_bellows() {
        let dir = std::env::temp_dir().join(format!("omsi-humans-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
        write(
            "paths_a.cfg",
            "[pathpnt]\n1.2\n4\n0.4\n[pathpnt]\n0\n4\n0.5\n[pathpnt]\n0\n0\n0.5\n[pathpnt]\n0\n-4.2\n0.6\n[pathpnt]\n1.2\n0\n0.4\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n[pathlink]\n2\n3\n[pathlink]\n2\n4\n",
        );
        write(
            "cabin_a.cfg",
            "[entry]\n0\n[exit]\n4\n[linkToPrevVeh]\n3\n[passpos]\n-0.5\n2\n1.0\n0.45\n0\n",
        );
        write(
            "paths_b.cfg",
            "[pathpnt]\n0\n3.6\n0.6\n[pathpnt]\n0\n0\n0.6\n[pathpnt]\n1.2\n0\n0.4\n[pathpnt]\n0\n-2\n0.6\n[pathlink]\n0\n1\n[pathlink]\n1\n2\n[pathlink]\n1\n3\n",
        );
        write(
            "cabin_b.cfg",
            "[exit]\n2\n[linkToNextVeh]\n0\n[passpos]\n-0.5\n-2\n1.1\n0.45\n0\n",
        );
        let def = |cabin: &str, paths: &str| omsi_vehicle::Vehicle {
            path: dir.join("bus.bus"),
            passenger_cabin: Some(cabin.into()),
            paths: Some(paths.into()),
            bounding_box: Some([2.5, 9.0, 3.0, 0.0, 0.0, 1.5]),
            ..Default::default()
        };
        let (front, rear) = (
            def("cabin_a.cfg", "paths_a.cfg"),
            def("cabin_b.cfg", "paths_b.cfg"),
        );
        let (back, own) = (Vec3::new(0.0, -4.3, 0.3), Vec3::new(0.0, 4.0, 0.3));
        let offset = back - own;
        let cabin =
            Cabin::load_train(&[(&front, Vec3::ZERO, f32::INFINITY), (&rear, offset, back.y)])
                .expect("cabin");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(cabin.parts.len(), 2);
        assert_eq!(cabin.points.len(), 9);
        assert_eq!(
            (cabin.entries.len(), cabin.exits.len(), cabin.seats.len()),
            (1, 2, 2)
        );
        assert!(
            (cabin.exits[1].inside - Vec3::new(1.2, -8.3, 0.4)).length() < 1e-4,
            "{:?}",
            cabin.exits[1].inside
        );
        let seat = &cabin.seats[1];
        assert!((seat.pos.y + 10.3).abs() < 1e-4);
        let to = cabin
            .omsi_nearest(seat.floor, &cabin.all_points(), false, false, None, None)
            .unwrap();
        let route = walk(&cabin, cabin.entries[0].point.unwrap(), to);
        assert!(
            route.iter().any(|p| (p.y + 4.2).abs() < 1e-4)
                && route.iter().any(|p| (p.y + 4.7).abs() < 1e-4),
            "{route:?}"
        );
        let exit = cabin.omsi_nearest(seat.floor, &cabin.exit_points(), false, false, None, None);
        assert_eq!(exit, cabin.exits[1].point);
        // bent 30 degrees about the coupling, a walk down the aisle moves on without a jump
        let lead_rot = Mat4::IDENTITY;
        let bent = 30.0f64;
        let rot = Mat4::from_rotation_z((-bent).to_radians() as f32);
        let pos = back.as_dvec3() - rot.transform_point3(own).as_dvec3();
        let frames = [PartFrame {
            pos,
            rot,
            heading: bent,
            offset,
            joint_y: back.y,
            half: DVec2::new(1.25, 4.5),
            centre: DVec2::ZERO,
        }];
        let at_joint = Vec3::new(0.6, back.y, 0.5);
        let rear_frame = pos + rot.transform_point3(at_joint - offset).as_dvec3();
        assert!((rear_frame - at_joint.as_dvec3()).length() > 0.3);
        let mut last = train_point(DVec3::ZERO, &lead_rot, &frames, Vec3::new(0.6, 0.0, 0.5));
        for k in 1..=100 {
            let y = -(k as f32) * 0.1;
            let p = train_point(DVec3::ZERO, &lead_rot, &frames, Vec3::new(0.6, y, 0.5));
            assert!(
                (p - last).length() < 0.14,
                "a jump of {:.3} m at y {y}",
                (p - last).length()
            );
            last = p;
        }
        let ahead = train_point(DVec3::ZERO, &lead_rot, &frames, Vec3::new(1.0, -1.0, 0.5));
        assert!((ahead - DVec3::new(1.0, -1.0, 0.5)).length() < 1e-4);
        let behind_joint = Vec3::new(1.0, -9.0, 0.5);
        let p = train_point(DVec3::ZERO, &lead_rot, &frames, behind_joint);
        assert!(
            (p - (pos + rot.transform_point3(behind_joint - offset).as_dvec3())).length() < 1e-4
        );
        assert!((train_heading(0.0, &frames, behind_joint) - bent).abs() < 1e-9);
        assert!(
            (train_heading(0.0, &frames, Vec3::new(0.0, back.y, 0.5)) - bent * 0.5).abs() < 1e-9
        );
    }

    #[test]
    fn footsteps_come_from_the_links_step_sound_pack() {
        let Some(def) = stock_bus("MAN_SD200/MAN_SD80.bus") else {
            return;
        };
        let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
        let first = |p: Vec3| {
            let pts = &cabin.points;
            let d = |l: &(i32, i32, bool)| {
                let (a, b) = (pts[l.0 as usize], pts[l.1 as usize]);
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                (a + ab * t - p).length()
            };
            let l = (0..cabin.links.len())
                .min_by(|&x, &y| d(&cabin.links[x]).total_cmp(&d(&cabin.links[y])))?;
            cabin.link_pack[l].map(|k| cabin.step_packs[k][0].to_ascii_lowercase())
        };
        assert_eq!(
            first(Vec3::new(-0.89, -1.61, 1.63)).as_deref(),
            Some("step_st_01.wav"),
            "the rear stairs"
        );
        assert_eq!(
            first(Vec3::new(0.0, 4.35, 2.5)).as_deref(),
            Some("step_ov_01.wav"),
            "the upper deck's front"
        );
        assert_eq!(
            first(Vec3::new(0.0, 0.84, 0.57)).as_deref(),
            Some("step_01.wav"),
            "the aisle below"
        );
    }

    #[test]
    fn double_decker_exits_are_reached_down_the_stairs() {
        let Some(def) = stock_bus("MAN_SD202/MAN_D92.bus") else {
            return;
        };
        let cabin = Cabin::load_train(&[(&def, Vec3::ZERO, f32::INFINITY)]).expect("cabin");
        assert_eq!(cabin.exits.len(), 2);
        let exit = &cabin.exits[1];
        let upstairs = cabin
            .omsi_nearest(
                Vec3::new(0.0, -1.8, 2.46),
                &cabin.all_points(),
                false,
                false,
                None,
                None,
            )
            .unwrap();
        assert!((cabin.points[upstairs].z - 2.46).abs() < 0.1);
        let route = walk(&cabin, upstairs, exit.point.unwrap());
        assert!(
            route.iter().any(|p| (p.z - 1.82).abs() < 0.01)
                && route.iter().any(|p| (p.z - 1.205).abs() < 0.01),
            "{route:?}"
        );
    }

    #[test]
    fn routes_follow_the_link_order_and_one_way_links() {
        // 0 - 1 - 2, and 2 -> 0 one way
        let r = build_routes(3, &[(0, 1, false), (1, 2, false), (2, 0, true)]);
        let next = |a: usize, b: usize| r[a].iter().find(|l| l.reach.contains(&b)).map(|l| l.to);
        assert_eq!(next(0, 2), Some(1));
        assert_eq!(next(1, 0), Some(0));
        assert_eq!(next(1, 2), Some(2));
    }
}
