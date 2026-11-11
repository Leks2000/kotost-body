// Kotost Bust (id kotost_body): a Horizon native mod (horizon-mixins support module, put into horizon-client-lib's root).
// 1) The bust item's two parts (`bust_left`, `bust_right`) jiggle on a damped spring, driven only by walking
//    (the leg cycle) and by the body's vertical acceleration (jump take-off, landing, steps): standing still,
//    they settle and stay still.
// 2) First person is its own mod (Kotost First Person): it draws the player model once more, so the bust and
//    the hourglass body show on it by themselves; nothing is shared between the two.
// 3) Every player the settings pick (slim skins by default) gets the bust drawn by the mod itself, textured from
//    their own skin: no cosmetics item needed, so everyone with the mod sees it on everyone.
// 4) Every bust wearer gets an hourglass torso (narrow waist, wider hips) with their skin stretched over it,
//    in place of the game's box; another mod's dress can take the bust over (`cover`, `bust_part_pose`).
// Numbers can be tuned while playing: `kotost-body.txt` in HORIZON_HOME (key = value), re-read when it changes.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

/// One frame's bust offsets of the wearer being drawn (model pixels, + is down), [left, right].
#[derive(Clone, Copy, Debug, Default)]
pub struct Jiggle {
    pub drop: [f32; 2],
    pub sway: [f32; 2],
}

#[derive(Clone, Copy, Default)]
struct Spring {
    x: f32,
    v: f32,
}

impl Spring {
    fn step(&mut self, target: f32, k: f32, c: f32, dt: f32) {
        let a = -k * (self.x - target) - c * self.v;
        self.v += a * dt;
        self.x += self.v * dt;
    }
}

#[derive(Clone, Copy, Default)]
struct Wearer {
    seen: bool,
    t: f32,
    age: u32,
    vy: f64,
    /// The last footfall seen (the leg cycle's half-turns).
    step: i64,
    /// Left and right each on a spring of their own, a little out of tune with each other.
    bounce: [Spring; 2],
    sway: [Spring; 2],
    /// xorshift state: every kick a little different, never the same twice.
    rng: u64,
}

/// A number in [-1, 1] (xorshift64*).
fn random(rng: &mut u64) -> f32 {
    let mut x = if *rng == 0 { 0x9E37_79B9_7F4A_7C15 } else { *rng };
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *rng = x;
    let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40;
    r as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
}

/// Who gets the bust drawn from their skin (`skin_bust` in the file).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Auto {
    /// Nobody: only players wearing a bust cosmetic item.
    Off,
    /// Only the player at this game (their own model), plus `bust_names`, minus `bust_skip`: the default,
    /// so other players are left as they look unless the settings ask for them.
    Me,
    /// Players with a slim (3 px arm) skin, plus `bust_names`, minus `bust_skip`.
    Slim,
    /// Only the players in `bust_names`.
    Names,
    /// Every player, minus `bust_skip`.
    All,
}

#[derive(Clone, Copy, Debug)]
pub struct Tune {
    pub bust: bool,
    pub auto: Auto,
    /// Writes why the bust was or wasn't drawn to `kotost-body-debug.log` (at most once a second per player).
    pub debug: bool,
    /// Spring stiffness (per tick²) and damping (per tick) of the up-down motion.
    pub k: f32,
    pub c: f32,
    /// Walking: the downward kick (px/tick) each footfall gives the bust at a walk (more running).
    pub walk: f32,
    /// Walking: the sideways kick (px/tick) of each footfall, one foot then the other.
    pub sway: f32,
    /// How much more a run kicks than a walk: the kick grows as (speed / walking speed) ^ run_pow.
    pub run_pow: f32,
    /// How different the two sides and each kick are (0: in step, 0.25: a little, 1: a lot).
    pub vary: f32,
    pub sway_k: f32,
    pub sway_c: f32,
    /// Vertical acceleration (blocks/tick²) to spring speed (px/tick).
    pub jump: f32,
    /// The drop's limits (px).
    pub up_max: f32,
    pub down_max: f32,
    /// Spring displacement to pose: pixels moved down and degrees turned, per pixel.
    pub move_px: f32,
    pub turn_deg: f32,
    /// The hourglass body for every bust wearer: the torso narrows to `waist` and widens to `hips`
    /// (half-widths, px; the game's torso is 4), their skin stretched over it. Off: the game's box.
    pub hourglass: bool,
    pub waist: f32,
    pub hips: f32,
    /// Degrees both arms hang further out, so the wider hips don't cut into them.
    pub hip_arms: f32,
}

impl Default for Tune {
    fn default() -> Self {
        Tune {
            bust: true,
            auto: Auto::Me,
            debug: false,
            k: 2.0,
            c: 0.75,
            walk: 2.45,
            sway: 0.12,
            run_pow: 0.4,
            vary: 0.25,
            sway_k: 2.0,
            sway_c: 1.0,
            jump: 0.83,
            up_max: 3.0,
            down_max: 4.0,
            move_px: 0.45,
            turn_deg: 5.0,
            hourglass: true,
            waist: 3.3,
            hips: 4.3,
            hip_arms: 3.0,
        }
    }
}

// Drawing runs on one thread; the locks are never held across a call into the game.
static NESTED: AtomicBool = AtomicBool::new(false);
static CURRENT: Mutex<Option<Jiggle>> = Mutex::new(None);
static WEARERS: Mutex<Option<HashMap<u32, Wearer>>> = Mutex::new(None);
static TUNE: Mutex<Option<TuneCache>> = Mutex::new(None);
/// The wearer `CURRENT` belongs to (`u32::MAX`: the player's own model).
static CURRENT_KEY: Mutex<u32> = Mutex::new(0);
/// Wearers seen with a bust cosmetic item lately: the mod draws them no second one.
static ITEM_BUSTS: Mutex<Option<HashMap<u32, Instant>>> = Mutex::new(None);
/// Players' names by wearer key, asked of the game at most once a second.
static NAMES: Mutex<Option<(Instant, HashMap<u32, String>)>> = Mutex::new(None);

struct TuneCache {
    tune: Tune,
    /// `bust_names` and `bust_skip`, lower case.
    names: Vec<String>,
    skip: Vec<String>,
    stamp: Option<SystemTime>,
    checked: Instant,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn nested() -> bool {
    NESTED.load(Ordering::Relaxed)
}

pub fn set_nested(v: bool) {
    NESTED.store(v, Ordering::Relaxed);
}

/// The bust offsets of the wearer whose cosmetics are being drawn.
pub fn current() -> Option<Jiggle> {
    *lock(&CURRENT)
}

/// The settings file: `config/kotost-body.txt` in the game's folder (`--home`, `HORIZON_HOME` or the
/// platform's data folder), where native mods keep theirs; one beside `options.txt` (0.1) still counts.
fn config_path() -> std::path::PathBuf {
    let home = crate::logging::game_home();
    let old = home.join("kotost-body.txt");
    if old.is_file() {
        return old;
    }
    home.join("config").join("kotost-body.txt")
}

/// What a missing settings file is made with: every key, its default and what it does.
const DEFAULT_FILE: &str = r#"# Kotost Body settings. The game re-reads this file by itself (twice a second): no restart needed.
# key = value; anything after # is a comment; delete a line to get its default back.

# --- who gets the bust (drawn by the mod from the player's own skin) ---
skin_bust = me     # me: only you | off: nobody | names: only bust_names | slim: every slim skin (3 px arms) | all: everyone
bust_names =       # player names, comma separated: always get it, even with a wide (Steve) skin
bust_skip =        # player names, comma separated: never get it

# --- the body of every bust wearer: an hourglass (narrow waist, wider hips), their own skin stretched over it ---
hourglass = 1      # 0: the game's box torso
waist = 3.3        # half-width of the waist, px (the game's torso is 4)
hips = 4.3         # half-width of the hips, px
hip_arms = 3       # degrees the arms hang further out (so the hips don't cut into them)

# --- bust motion: only while walking/running and on jumps/landings, still when standing ---
bust = 1           # 0: no motion
walk = 2.45        # downward kick of each step (running kicks harder)
run_pow = 0.4      # how much harder running is than walking (0 = the same)
vary = 0.25        # how different left and right and each kick are (0 = in sync)
sway = 0.12        # sideways kick of each step
jump = 0.83        # kick of a jump or landing
k = 2.0            # spring stiffness (higher = quicker)
c = 0.75           # damping (lower = shakes longer)
sway_k = 2.0       # sideways spring stiffness
sway_c = 1.0       # sideways damping
down_max = 4.0     # limit down, px
up_max = 3.0       # limit up, px
move_px = 0.45     # up-down shift per spring px
turn_deg = 5       # swing about the top edge, degrees per spring px

"#;

fn names_of(value: &str) -> Vec<String> {
    value
        .split([',', ' ', ';'])
        .map(|n| n.trim().to_lowercase())
        .filter(|n| !n.is_empty())
        .collect()
}

fn parse(text: &str) -> (Tune, Vec<String>, Vec<String>) {
    let mut t = Tune::default();
    let (mut names, mut skip) = (Vec::new(), Vec::new());
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "skin_bust" => {
                t.auto = match value.to_lowercase().as_str() {
                    "0" | "off" | "no" | "none" => Auto::Off,
                    "names" | "list" => Auto::Names,
                    "all" | "everyone" => Auto::All,
                    "slim" => Auto::Slim,
                    _ => Auto::Me,
                };
                continue;
            }
            "bust_names" => {
                names = names_of(value);
                continue;
            }
            "bust_skip" => {
                skip = names_of(value);
                continue;
            }
            _ => {}
        }
        let on = matches!(value, "1" | "on" | "true" | "yes");
        let Ok(n) = value.parse::<f32>() else {
            match key {
                "bust" => t.bust = on,
                "debug" => t.debug = on,
                "hourglass" => t.hourglass = on,
                _ => {}
            }
            continue;
        };
        if !n.is_finite() {
            continue;
        }
        match key {
            "bust" => t.bust = n != 0.0,
            "debug" => t.debug = n != 0.0,
            "k" => t.k = n.clamp(0.05, 4.0),
            "c" => t.c = n.clamp(0.0, 4.0),
            "walk" => t.walk = n.clamp(0.0, 4.0),
            "sway" => t.sway = n.clamp(0.0, 3.0),
            "run_pow" => t.run_pow = n.clamp(0.0, 4.0),
            "vary" => t.vary = n.clamp(0.0, 1.0),
            "sway_k" => t.sway_k = n.clamp(0.05, 4.0),
            "sway_c" => t.sway_c = n.clamp(0.0, 4.0),
            "jump" => t.jump = n.clamp(0.0, 3.0),
            "up_max" => t.up_max = n.clamp(0.0, 4.0),
            "down_max" => t.down_max = n.clamp(0.0, 5.0),
            "move_px" => t.move_px = n.clamp(0.0, 2.0),
            "turn_deg" => t.turn_deg = n.clamp(0.0, 20.0),
            "hourglass" => t.hourglass = n != 0.0,
            "waist" => t.waist = n.clamp(2.0, 4.0),
            "hips" => t.hips = n.clamp(3.5, 5.0),
            "hip_arms" => t.hip_arms = n.clamp(0.0, 15.0),
            _ => {}
        }
    }
    (t, names, skip)
}

/// The settings file's cache, read again when it changed (checked at most twice a second).
fn cached<R>(f: impl FnOnce(&TuneCache) -> R) -> R {
    let mut cache = lock(&TUNE);
    let now = Instant::now();
    let due = cache
        .as_ref()
        .is_none_or(|c| now.duration_since(c.checked) > Duration::from_millis(500));
    if due {
        let path = config_path();
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        match cache.as_mut() {
            Some(c) if c.stamp == stamp => c.checked = now,
            _ => {
                let (tune, names, skip) = match std::fs::read_to_string(&path) {
                    Ok(text) => parse(&text),
                    Err(_) => {
                        // First run: write the file with its defaults, to be edited while playing.
                        if !path.exists()
                            && let Some(dir) = path.parent()
                            && std::fs::create_dir_all(dir).is_ok()
                        {
                            let _ = std::fs::write(&path, DEFAULT_FILE);
                        }
                        (Tune::default(), Vec::new(), Vec::new())
                    }
                };
                *cache = Some(TuneCache {
                    tune,
                    names,
                    skip,
                    stamp,
                    checked: now,
                });
            }
        }
    }
    f(cache.as_ref().expect("filled above"))
}

/// The tuning, from `kotost-body.txt` (defaults without it).
pub fn tune() -> Tune {
    cached(|c| c.tune)
}

/// Called as a wearer's cosmetics start being drawn: steps their bust spring to now and keeps the
/// offsets for `apply`. Drawing the same wearer twice in a frame (shadow, preview) steps nothing more.
pub fn wearer(m: &crate::mobs::Mob, own: bool, alpha: f32) {
    let tune = tune();
    let key = if own { u32::MAX } else { m.id };
    *lock(&CURRENT_KEY) = key;
    if !tune.bust {
        *lock(&CURRENT) = None;
        return;
    }
    let s = crate::mobs::anim::state(m, alpha);
    let t = m.age as f32 + alpha;
    let vy = m.position.y - m.previous.y;
    // How fast the body goes over the ground, against a walk (4.317 m/s): running shakes more.
    let (dx, dz) = (m.position.x - m.previous.x, m.position.z - m.previous.z);
    let pace = ((dx * dx + dz * dz).sqrt() as f32 / 0.2158).clamp(0.0, 1.5);
    let jiggle = {
        let mut guard = lock(&WEARERS);
        let all = guard.get_or_insert_with(HashMap::new);
        if all.len() > 256 {
            all.clear();
        }
        let w = all.entry(key).or_default();
        let dt = t - w.t;
        if !w.seen || !(0.0..=10.0).contains(&dt) {
            // New, back after a while, or time went backwards (a new world): start at rest.
            *w = Wearer {
                seen: true,
                t,
                age: m.age,
                vy,
                step: (s.pos * 0.6662 / std::f32::consts::PI).floor() as i64,
                rng: 0x9E37_79B9_7F4A_7C15 ^ (u64::from(key) << 20) ^ u64::from(m.age),
                ..Wearer::default()
            };
        } else {
            // A new tick: the body's vertical acceleration kicks the bust (it lags behind).
            if m.age != w.age {
                if m.age.wrapping_sub(w.age) <= 2 {
                    let a = ((vy - w.vy) as f32).clamp(-0.8, 0.8);
                    for side in 0..2 {
                        let r = random(&mut w.rng);
                        w.bounce[side].v += a * 16.0 * tune.jump * (1.0 + 0.5 * tune.vary * r);
                    }
                }
                w.age = m.age;
                w.vy = vy;
            }
            if dt > 0.0 {
                // Walking: each footfall (the legs at their widest, every half-turn of the leg cycle)
                // kicks the bust down like a small landing, and sideways, one foot then the other;
                // the spring does the rest. Standing, nothing kicks it and it settles.
                let step = (s.pos * 0.6662 / std::f32::consts::PI).floor() as i64;
                if step != w.step {
                    let amount = s.speed.clamp(0.0, 1.0) * pace.powf(tune.run_pow);
                    if amount > 0.05 {
                        let foot = if step.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
                        for side in 0..2 {
                            let r = random(&mut w.rng);
                            w.bounce[side].v += tune.walk * amount * (1.0 + tune.vary * r);
                            let r = random(&mut w.rng);
                            w.sway[side].v += tune.sway * amount * foot * (1.0 + tune.vary * r);
                        }
                    }
                    w.step = step;
                }
                let steps = (dt / 0.25).ceil().max(1.0);
                let h = dt / steps;
                // Left a touch stiffer, right a touch softer: they drift in and out of step.
                let detune = 0.2 * tune.vary;
                for (side, tone) in [(0, 1.0 + detune), (1, 1.0 - detune)] {
                    for _ in 0..steps as u32 {
                        w.bounce[side].step(0.0, tune.k * tone, tune.c, h);
                        w.sway[side].step(0.0, tune.sway_k * tone, tune.sway_c, h);
                    }
                    w.bounce[side].x = w.bounce[side].x.clamp(-tune.up_max, tune.down_max);
                    w.sway[side].x = w.sway[side].x.clamp(-1.5, 1.5);
                }
                w.t = t;
            }
        }
        Jiggle {
            drop: [w.bounce[0].x, w.bounce[1].x],
            sway: [w.sway[0].x, w.sway[1].x],
        }
    };
    *lock(&CURRENT) = Some(jiggle);
}

/// Moves the bust parts of an item's poses by the current offsets.
pub fn apply(p: &mut crate::mobs::anim::Poses, j: Jiggle) {
    let tune = tune();
    for (side, name) in ["bust_left", "bust_right"].into_iter().enumerate() {
        if !p.has(name) {
            continue;
        }
        jiggle_pose(p.get(name), side, j, &tune);
    }
}

/// One side's offsets (`side` 0: left, 1: right) on its pose: down/up (and a little turn about the top),
/// and sideways with the steps.
fn jiggle_pose(q: &mut [f32; 9], side: usize, j: Jiggle, tune: &Tune) {
    q[0] += j.sway[side];
    q[1] += j.drop[side] * tune.move_px;
    q[3] += (j.drop[side] * tune.turn_deg).to_radians();
}

/// For another mod drawing the bust itself (a dress over it): steps the wearer's springs to now, as the
/// mod's own bust does, and gives this frame's offsets. Cheap to call again in the same frame.
pub fn jiggle(m: &crate::mobs::Mob, own: bool, alpha: f32) -> Jiggle {
    wearer(m, own, alpha);
    current().unwrap_or_default()
}

/// One breast's frame on the body bone (`side` 0: left, +x; 1: right), moved by `j`: the pose of the
/// part, then `bust_box_pose` inside it, where the breast is the box x 0..BUST_W (the right one
/// -BUST_W..0), y 0..BUST_H (down), z -BUST_D..0 (its front at -BUST_D).
pub fn bust_part_pose(side: usize, j: Jiggle) -> [f32; 9] {
    let s = if side == 0 { 1.0f32 } else { -1.0 };
    let mut q = [
        0.0,
        BUST_TOP,
        -2.0,
        0.0,
        (BUST_YAW * s).to_radians(),
        (-BUST_ROLL * s).to_radians(),
        1.0,
        1.0,
        1.0,
    ];
    jiggle_pose(&mut q, side, j, &tune());
    q
}

/// The box's pose inside `bust_part_pose`: tilted about its inner-back-top edge, sunk so the top-front
/// edge lies on the chest.
pub fn bust_box_pose() -> [f32; 9] {
    let tilt = BUST_TILT.to_radians();
    [0.0, BUST_D * tilt.sin(), BUST_D * tilt.cos(), -tilt, 0.0, 0.0, 1.0, 1.0, 1.0]
}

// --- the hourglass body ----------------------------------------------------------------------------

/// By mob id: drawn with the hourglass body lately (the skin-bust hook decides, the model's poses read it).
static SHAPED: Mutex<Option<HashMap<u32, Instant>>> = Mutex::new(None);
/// By mob id: wearing a bust lately (the item or the one from the skin).
static BUSTY: Mutex<Option<HashMap<u32, Instant>>> = Mutex::new(None);
/// By wearer key: another mod draws this wearer's bust in its own clothes; ours stays hidden.
static COVERED: Mutex<Option<HashMap<u32, Instant>>> = Mutex::new(None);

thread_local! {
    /// The mob whose model `anim::setup` is posing on this thread.
    static SETUP_ID: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

fn mark(map: &Mutex<Option<HashMap<u32, Instant>>>, key: u32, on: bool) {
    let mut guard = lock(map);
    let all = guard.get_or_insert_with(HashMap::new);
    if all.len() > 256 {
        all.clear();
    }
    if on {
        all.insert(key, Instant::now());
    } else {
        all.remove(&key);
    }
}

fn marked(map: &Mutex<Option<HashMap<u32, Instant>>>, key: u32, within: Duration) -> bool {
    lock(map)
        .as_ref()
        .and_then(|all| all.get(&key))
        .is_some_and(|t| t.elapsed() < within)
}

/// The skin-bust hook's verdict for the mob `id` this frame: hourglass body or not, a bust or not.
pub fn note_body(id: u32, shaped: bool, busty: bool) {
    mark(&SHAPED, id, shaped);
    mark(&BUSTY, id, busty);
}

/// Whether the mob `id` has the hourglass body.
pub fn shaped(id: u32) -> bool {
    marked(&SHAPED, id, Duration::from_millis(500))
}

/// Whether the mob `id` wears a bust (the item, or the one the mod draws from the skin).
pub fn bust_on(id: u32) -> bool {
    marked(&BUSTY, id, Duration::from_millis(500))
}

/// Another mod dresses this wearer's bust (`key`: `u32::MAX` for the player's own model) this frame:
/// neither the bust from the skin nor a bust item is drawn on them for the next half second.
pub fn cover(key: u32) {
    mark(&COVERED, key, true);
}

pub fn covered(key: u32) -> bool {
    marked(&COVERED, key, Duration::from_millis(500))
}

/// Whether the wearer whose cosmetics are being drawn has their bust covered by another mod.
pub fn current_covered() -> bool {
    covered(*lock(&CURRENT_KEY))
}

/// Whether the wearer `key` wore a bust item lately.
pub fn wears_item(key: u32) -> bool {
    wears_item_bust(key)
}

/// Chest items that are clothes, not armour (another mod's dress): under them the bust and the hourglass
/// body stay, as under an elytra.
static CLOTHES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Marks the chest item `id` (e.g. `kingdoms:mage_dress`) as clothes, not armour.
pub fn cloth(id: &str) {
    let mut all = lock(&CLOTHES);
    if !all.iter().any(|c| c == id) {
        all.push(id.to_owned());
    }
}

/// Whether the chest item `id` was marked as clothes.
pub fn is_cloth(id: &str) -> bool {
    lock(&CLOTHES).iter().any(|c| c == id)
}

/// `anim::setup`'s HEAD: the mob being posed.
pub fn setup_mob(m: &crate::mobs::Mob) {
    SETUP_ID.with(|c| c.set(Some(m.id)));
}

/// `player_model_visibility`'s HEAD: an hourglass player's model leaves its torso and jacket boxes
/// undrawn (the mod draws the hourglass in their place), and hangs the arms a little further out.
pub fn shape_poses(p: &mut crate::mobs::anim::Poses) {
    let Some(id) = SETUP_ID.with(std::cell::Cell::take) else {
        return;
    };
    if !shaped(id) {
        return;
    }
    p.skip_draw("body");
    p.skip_draw("jacket");
    let out = tune().hip_arms.to_radians();
    if out > 0.0 {
        if p.has("right_arm") {
            p.get("right_arm")[5] += out;
        }
        if p.has("left_arm") {
            p.get("left_arm")[5] -= out;
        }
    }
}

/// The torso's half-width (px) at `y` pixels below the neck: the chest as the game's, narrowing from
/// under the bust to the waist, widening to the hips. 4 everywhere with `hourglass = 0`.
pub fn half_width(y: f32) -> f32 {
    let t = tune();
    if !t.hourglass {
        return 4.0;
    }
    width_at(y, t.waist, t.hips)
}

/// The hourglass's waist and hips (half-widths, px) for `width_at`; none with `hourglass = 0`.
pub fn waist_hips() -> Option<(f32, f32)> {
    let t = tune();
    t.hourglass.then_some((t.waist, t.hips))
}

/// The hourglass torso's half-width at `y` pixels below the neck, for this waist and these hips.
pub fn width_at(y: f32, waist: f32, hips: f32) -> f32 {
    let s = |a: f32| {
        let a = a.clamp(0.0, 1.0);
        a * a * (3.0 - 2.0 * a)
    };
    if y <= 8.0 {
        4.0 + (waist - 4.0) * s((y - 5.0) / 3.0)
    } else {
        waist + (hips - waist) * s((y - 8.0) / 3.0)
    }
}

/// The hourglass torso (`grow` 0) or its jacket (`grow` 0.25, the skin's second layer 16 rows down): a
/// stack of one-pixel rows, each as wide as `width_at` there, every face cut from the skin where the game's
/// torso box takes it (front 20..28, sides 16..20 and 28..32, back 32..40, top and bottom at row 16).
/// `tex_h`: the texture's height in pixels (64 for a skin, 32 for an armour layer, laid out as the torso).
fn torso(grow: f32, rows_down: f32, waist: f32, hips: f32, tex_h: f32) -> Vec<crate::mobs::Polygon> {
    let d = 2.0 + grow;
    let w = |y: f32| width_at(y, waist, hips) + grow;
    let gy = |y: f32| {
        if y <= 0.0 {
            -grow
        } else if y >= 12.0 {
            12.0 + grow
        } else {
            y
        }
    };
    let face = |at: [[f32; 3]; 4], uv: [[f32; 2]; 4]| {
        let e1 = [at[1][0] - at[0][0], at[1][1] - at[0][1], at[1][2] - at[0][2]];
        let e2 = [at[2][0] - at[0][0], at[2][1] - at[0][1], at[2][2] - at[0][2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        crate::mobs::Polygon {
            normal: [n[0] / l, n[1] / l, n[2] / l],
            vertices: std::array::from_fn(|i| {
                [at[i][0], at[i][1], at[i][2], uv[i][0] / 64.0, (uv[i][1] + rows_down) / tex_h]
            }),
            cull_back: false,
        }
    };
    let mut out = Vec::new();
    for row in 0..12 {
        let (ya, yb) = (row as f32, row as f32 + 1.0);
        let (wa, wb) = (w(ya), w(yb));
        let (pa, pb) = (gy(ya), gy(yb));
        let (va, vb) = (20.0 + ya, 20.0 + yb);
        // The corners in the game's order for each face, so the texture lies as on its box.
        out.push(face(
            [[wa, pa, -d], [-wa, pa, -d], [-wb, pb, -d], [wb, pb, -d]],
            [[28.0, va], [20.0, va], [20.0, vb], [28.0, vb]],
        ));
        out.push(face(
            [[-wa, pa, d], [wa, pa, d], [wb, pb, d], [-wb, pb, d]],
            [[40.0, va], [32.0, va], [32.0, vb], [40.0, vb]],
        ));
        out.push(face(
            [[-wa, pa, -d], [-wa, pa, d], [-wb, pb, d], [-wb, pb, -d]],
            [[20.0, va], [16.0, va], [16.0, vb], [20.0, vb]],
        ));
        out.push(face(
            [[wa, pa, d], [wa, pa, -d], [wb, pb, -d], [wb, pb, d]],
            [[32.0, va], [28.0, va], [28.0, vb], [32.0, vb]],
        ));
    }
    let (wt, wbm) = (w(0.0), w(12.0));
    let (top, bottom) = (gy(0.0), gy(12.0));
    out.push(face(
        [[wt, top, d], [-wt, top, d], [-wt, top, -d], [wt, top, -d]],
        [[28.0, 16.0], [20.0, 16.0], [20.0, 20.0], [28.0, 20.0]],
    ));
    out.push(face(
        [[wbm, bottom, -d], [-wbm, bottom, -d], [-wbm, bottom, d], [wbm, bottom, d]],
        [[36.0, 16.0], [28.0, 16.0], [28.0, 20.0], [36.0, 20.0]],
    ));
    out
}

/// The hourglass torso on the body bone, its jacket too when `jacket`; the skin's own UV (0..1 of the
/// 64x64 skin). Built again only when the waist or hips change.
pub fn hourglass_body(jacket: bool) -> std::sync::Arc<crate::mobs::Part> {
    let t = tune();
    shaped_part(t.waist, t.hips, if jacket { 1 } else { 0 })
}

/// The chestplate over the hourglass body: the armour layer's torso (64x32, a pixel out like the game's
/// chestplate box) following the waist and hips, so armour keeps the figure.
pub fn armor_body() -> std::sync::Arc<crate::mobs::Part> {
    let t = tune();
    shaped_part(t.waist, t.hips, 2)
}

/// `kind` 0: the skin's torso, 1: with its jacket, 2: the chestplate's.
fn shaped_part(waist: f32, hips: f32, kind: u8) -> std::sync::Arc<crate::mobs::Part> {
    static CACHE: Mutex<Vec<((u32, u32, u8), std::sync::Arc<crate::mobs::Part>)>> = Mutex::new(Vec::new());
    let key = (waist.to_bits(), hips.to_bits(), kind);
    let mut cache = lock(&CACHE);
    if let Some((_, part)) = cache.iter().find(|(k, _)| *k == key) {
        return part.clone();
    }
    let polygons = match kind {
        2 => torso(ARMOR_GROW, 0.0, waist, hips, 32.0),
        _ => {
            let mut p = torso(0.0, 0.0, waist, hips, 64.0);
            if kind == 1 {
                p.extend(torso(0.25, 16.0, waist, hips, 64.0));
            }
            p
        }
    };
    let part = std::sync::Arc::new(crate::mobs::Part {
        name: "kotost_torso".to_owned(),
        pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        polygons,
        children: Vec::new(),
    });
    if cache.len() > 8 {
        cache.clear();
    }
    cache.push((key, part.clone()));
    part
}

/// How far the chestplate stands off the body (the game's chestplate box: 1 px), and the cups over the
/// bust a little less (so they stay round, not blocky).
const ARMOR_GROW: f32 = 1.0;
const ARMOR_CUP_GROW: f32 = 0.6;

/// The armour layer texture (`entity/equipment/humanoid/...`) of a vanilla chestplate; none for another
/// mod's (its own model is left to the game).
pub fn armor_texture(chest: &str) -> Option<String> {
    let name = chest.strip_prefix("minecraft:")?;
    let material = name.strip_suffix("_chestplate")?;
    let asset = match material {
        "golden" => "gold",
        other => other,
    };
    Some(format!("entity/equipment/humanoid/{asset}"))
}

// --- the bust from the skin ------------------------------------------------------------------------

/// Called by the bust item's hook: the wearer being drawn wears a bust of their own.
pub fn mark_item_bust() {
    let key = *lock(&CURRENT_KEY);
    let mut guard = lock(&ITEM_BUSTS);
    let all = guard.get_or_insert_with(HashMap::new);
    if all.len() > 256 {
        all.clear();
    }
    all.insert(key, Instant::now());
}

fn wears_item_bust(key: u32) -> bool {
    lock(&ITEM_BUSTS)
        .as_ref()
        .and_then(|all| all.get(&key))
        .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
}

/// Players (lower case) who always get the bust and the hourglass body, whatever their skin and the
/// settings (`skin_bust = off` and `bust_skip` aside): the companion bot is a girl, no male version.
const FEMALE: &[&str] = &["bolvanchik"];

/// Whether the mod draws the bust on this player (`key`: `u32::MAX` for the player's own model).
/// `names` lists every player's name (asked only when the settings name players, once a second).
pub fn skin_bust_wanted(key: u32, slim: bool, names: impl FnOnce() -> Vec<(u32, String)>) -> bool {
    if wears_item_bust(key) {
        return false;
    }
    let auto = cached(|c| c.tune.auto);
    if auto == Auto::Off {
        return false;
    }
    // The names are always looked up (once a second): the companions (`FEMALE`) get the figure whatever
    // their skin.
    let name = {
        let mut guard = lock(&NAMES);
        let stale = guard.as_ref().is_none_or(|(at, known)| {
            let age = at.elapsed();
            age > Duration::from_secs(1) || (!known.contains_key(&key) && age > Duration::from_millis(250))
        });
        if stale {
            let known = names()
                .into_iter()
                .map(|(id, n)| (id, n.to_lowercase()))
                .collect();
            *guard = Some((Instant::now(), known));
        }
        guard.as_ref().and_then(|(_, known)| known.get(&key).cloned())
    };
    cached(|c| {
        let named = |list: &[String]| name.as_ref().is_some_and(|n| list.iter().any(|l| l == n));
        if named(&c.skip) {
            return false;
        }
        if name.as_deref().is_some_and(|n| FEMALE.contains(&n)) {
            return true;
        }
        match auto {
            Auto::Off => false,
            Auto::Names => named(&c.names),
            Auto::Me => key == u32::MAX || named(&c.names),
            Auto::Slim => slim || named(&c.names),
            Auto::All => true,
        }
    })
}

// The shape of tools/bust.py v4 (pixels): each breast its own box, the top flush with the chest and
// meeting at the centre, the bottoms a little apart.
pub const BUST_W: f32 = 4.0;
pub const BUST_H: f32 = 5.0;
pub const BUST_D: f32 = 4.0;
/// Degrees the bottom comes forward.
pub const BUST_TILT: f32 = 33.0;
/// Degrees each one leans outwards and turns its outer edge forward.
pub const BUST_ROLL: f32 = 6.0;
pub const BUST_YAW: f32 = 6.0;
/// The body row (pixels below the neck) where the top edge meets the chest.
pub const BUST_TOP: f32 = 1.0;

/// A box's six faces in vanilla order (down, up, west, north, east, south), as the cosmetics'
/// box UV makes them, but each face with its own rectangle of the 64x64 skin
/// (`[u0, v0, u1, v1]` in skin pixels; u1 < u0 flips it). `tex_h`: 64 for a skin, 32 for armour.
fn skin_box(
    origin: [f32; 3],
    size: [f32; 3],
    grow: f32,
    mirror: bool,
    rects: [[f32; 4]; 6],
    tex_h: f32,
) -> Vec<crate::mobs::Polygon> {
    let [w, h, d] = size;
    let (mut x0, y0, z0) = (origin[0] - grow, origin[1] - grow, origin[2] - grow);
    let (mut x1, y1, z1) = (origin[0] + w + grow, origin[1] + h + grow, origin[2] + d + grow);
    if mirror {
        std::mem::swap(&mut x0, &mut x1);
    }
    let t0 = [x0, y0, z0];
    let t1 = [x1, y0, z0];
    let t2 = [x1, y1, z0];
    let t3 = [x0, y1, z0];
    let l0 = [x0, y0, z1];
    let l1 = [x1, y0, z1];
    let l2 = [x1, y1, z1];
    let l3 = [x0, y1, z1];
    let corners: [([[f32; 3]; 4], [f32; 3]); 6] = [
        ([l1, l0, t0, t1], [0.0, -1.0, 0.0]),
        ([t2, t3, l3, l2], [0.0, 1.0, 0.0]),
        ([t0, l0, l3, t3], [-1.0, 0.0, 0.0]),
        ([t1, t0, t3, t2], [0.0, 0.0, -1.0]),
        ([l1, t1, t2, l2], [1.0, 0.0, 0.0]),
        ([l0, l1, l2, l3], [0.0, 0.0, 1.0]),
    ];
    corners
        .into_iter()
        .zip(rects)
        .map(|((at, mut normal), [a, b, c, e])| {
            let uvs = [[c, b], [a, b], [a, e], [c, e]];
            let mut vertices: [[f32; 5]; 4] = std::array::from_fn(|i| {
                let [x, y, z] = at[i];
                [x, y, z, uvs[i][0] / 64.0, uvs[i][1] / tex_h]
            });
            if mirror {
                vertices.reverse();
                normal[0] = -normal[0];
            }
            crate::mobs::Polygon {
                normal,
                vertices,
                cull_back: false,
            }
        })
        .collect()
}

/// One breast (`side` +1: the player's left, +x), every face cut from the body's own pixels: the
/// front from the chest under it (the clothes run on), the outer side from the body's side, the
/// inner side and the underside from the chest's innermost column and lowest row (the light shades
/// them). `kind` 1: the skin's second layer too, a quarter pixel out, as the body's jacket is; 2: the
/// chestplate's cup instead (the armour texture's chest, 64x32, a little out), so armour keeps the bust.
fn skin_breast(side: f32, kind: u8) -> crate::mobs::Part {
    let r0 = 20.0 + BUST_TOP;
    let r1 = r0 + BUST_H;
    let rects = if side > 0.0 {
        // Body front x 24..28 is the player's left half; their left side is x 28..32.
        [
            [24.0, r0, 28.0, r0 + 1.0],
            [24.0, r1 - 1.0, 28.0, r1],
            [24.0, r0, 25.0, r1],
            [24.0, r0, 28.0, r1],
            [28.0, r0, 32.0, r1],
            [24.0, r0, 28.0, r1],
        ]
    } else {
        // Mirrored: the right half x 20..24 and the right side x 16..20, flipped to read as on the body.
        [
            [24.0, r0, 20.0, r0 + 1.0],
            [24.0, r1 - 1.0, 20.0, r1],
            [23.0, r0, 24.0, r1],
            [24.0, r0, 20.0, r1],
            [20.0, r0, 16.0, r1],
            [24.0, r0, 20.0, r1],
        ]
    };
    let origin = if side > 0.0 {
        [0.0, 0.0, -BUST_D]
    } else {
        [-BUST_W, 0.0, -BUST_D]
    };
    let size = [BUST_W, BUST_H, BUST_D];
    let mut polygons = if kind == 2 {
        skin_box(origin, size, ARMOR_CUP_GROW, side < 0.0, rects, 32.0)
    } else {
        skin_box(origin, size, 0.0, side < 0.0, rects, 64.0)
    };
    if kind == 1 {
        // The jacket's pixels are 16 rows below the body's.
        let over = rects.map(|[a, b, c, e]| [a, b + 16.0, c, e + 16.0]);
        polygons.extend(skin_box(origin, size, 0.25, side < 0.0, over, 64.0));
    }
    let tilt = BUST_TILT.to_radians();
    let name = if side > 0.0 { "bust_left" } else { "bust_right" };
    // Tilted about its inner-back-top edge, sunk so the top-front edge lies on the chest.
    let tilted = crate::mobs::Part {
        name: format!("{name}_box"),
        pose: [
            0.0,
            BUST_D * tilt.sin(),
            BUST_D * tilt.cos(),
            -tilt,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
        ],
        polygons,
        children: Vec::new(),
    };
    crate::mobs::Part {
        name: name.to_owned(),
        pose: [
            0.0,
            BUST_TOP,
            -2.0,
            0.0,
            (BUST_YAW * side).to_radians(),
            (-BUST_ROLL * side).to_radians(),
            1.0,
            1.0,
            1.0,
        ],
        polygons: Vec::new(),
        children: vec![tilted],
    }
}

/// The bust drawn from a skin, on the body bone: with the jacket layer over it or without.
pub fn skin_bust(jacket: bool) -> &'static crate::mobs::Part {
    static PLAIN: std::sync::OnceLock<crate::mobs::Part> = std::sync::OnceLock::new();
    static JACKET: std::sync::OnceLock<crate::mobs::Part> = std::sync::OnceLock::new();
    let make = |jacket: bool| crate::mobs::Part {
        name: "kotost_bust".to_owned(),
        pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        polygons: Vec::new(),
        children: vec![skin_breast(1.0, u8::from(jacket)), skin_breast(-1.0, u8::from(jacket))],
    };
    if jacket {
        JACKET.get_or_init(|| make(true))
    } else {
        PLAIN.get_or_init(|| make(false))
    }
}

/// The chestplate's cups over the bust (the armour texture), on the body bone; posed like the skin bust.
pub fn armor_bust() -> &'static crate::mobs::Part {
    static CUPS: std::sync::OnceLock<crate::mobs::Part> = std::sync::OnceLock::new();
    CUPS.get_or_init(|| crate::mobs::Part {
        name: "kotost_bust".to_owned(),
        pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        polygons: Vec::new(),
        children: vec![skin_breast(1.0, 2), skin_breast(-1.0, 2)],
    })
}

/// With `debug = 1`: one line per player and second, saying what the bust hook did.
pub fn debug(key: u32, what: &str) {
    static LAST: Mutex<Option<HashMap<u32, Instant>>> = Mutex::new(None);
    if !tune().debug {
        return;
    }
    {
        let mut guard = lock(&LAST);
        let all = guard.get_or_insert_with(HashMap::new);
        if all.get(&key).is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return;
        }
        all.insert(key, Instant::now());
    }
    use std::io::Write;
    let path = crate::logging::game_home().join("kotost-body-debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{key}: {what}");
    }
}
