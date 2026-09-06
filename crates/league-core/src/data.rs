//! Static game data: the lane, the five champions, the items, the runes,
//! the summoner spells. Numbers live here and nowhere else; the sim reads
//! this table, the page shows it through the client's `data_json`.

/// Half-length of the lane: blue core at x=-62, red core at x=+62.
pub const CORE_X: f32 = 62.0;
/// Half-width of the walkable field; the lane corridor is narrower.
pub const FIELD_Z: f32 = 40.0;
/// Half-width of the drawn lane corridor (minions march inside it).
pub const LANE_Z: f32 = 7.0;
/// Court positions: one on each side of the lane, off the corridor.
pub const COURT_POS: [[f32; 2]; 2] = [[0.0, 16.0], [0.0, -16.0]];
/// A fountain is this close to your own core.
pub const FOUNTAIN_R: f32 = 7.0;
pub const CORE_HP: f32 = 3200.0;
pub const COURT_HP: f32 = 1400.0;
/// Seconds a boon lasts and how long a dead court sleeps.
pub const BOON_SECS: f32 = 100.0;
pub const COURT_RESPAWN: f32 = 150.0;
/// Champion level cap; one point per level.
pub const MAX_LEVEL: u8 = 12;
/// XP needed to go from `level` to `level + 1`.
pub const fn xp_needed(level: u8) -> f32 {
    150.0 + 85.0 * (level as f32 - 1.0)
}
/// Seconds before a wave leaves the fountain (first wave at 10 s, then 30 s).
pub const WAVE_FIRST: f32 = 10.0;
pub const WAVE_EVERY: f32 = 30.0;
/// Alive minions per team; a full lane delays the next wave.
pub const MINION_CAP: usize = 16;
/// Champion levels that unlock rank 1/2/3 of the ultimate.
pub const R_LEVELS: [u8; 3] = [6, 9, 12];
/// Select phase length, and how long the result screen holds.
pub const SELECT_SECS: f32 = 60.0;
pub const RESULT_SECS: f32 = 12.0;
/// Passive gold and XP per second.
pub const GOLD_PER_SEC: f32 = 1.2;
pub const XP_PER_SEC: f32 = 2.0;
/// Start-of-match gold.
pub const START_GOLD: u32 = 500;

/// Everything one ability costs, per rank. `desc` is the page's kit text;
/// damage curves are computed in `sim` so they can see level and stats.
#[derive(Clone, Copy, Debug)]
pub struct Ability {
    pub name: &'static str,
    pub desc: &'static str,
    pub mana: [f32; 3],
    pub cd: [f32; 3],
}

const fn ab(name: &'static str, desc: &'static str, mana: [f32; 3], cd: [f32; 3]) -> Ability {
    Ability {
        name,
        desc,
        mana,
        cd,
    }
}

/// A champion's whole table row. `atk_style` picks the client's auto-attack
/// visual: 0 melee arc, 1 drones, 2 bolt, 3 flame slash, 4 hook.
#[derive(Clone, Copy, Debug)]
pub struct ChampDef {
    pub key: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub hp0: f32,
    pub hp_l: f32,
    pub mn0: f32,
    pub mn_l: f32,
    pub ms: f32,
    pub ad0: f32,
    pub ad_l: f32,
    pub ap0: f32,
    pub ap_l: f32,
    pub range: f32,
    pub atk_cd: f32,
    pub atk_style: u8,
    pub colour: [f32; 3],
    pub q: Ability,
    pub w: Ability,
    pub e: Ability,
    pub r: Ability,
}

pub const SWARM: u8 = 0;
pub const KNIGHT: u8 = 1;
pub const HALLOW: u8 = 2;
pub const MAW: u8 = 3;
pub const TESSERA: u8 = 4;
pub const CHAMPS: [ChampDef; 5] = [
    ChampDef {
        key: "swarm",
        name: "SW4RM",
        title: "the AI Swarm",
        hp0: 560.0,
        hp_l: 60.0,
        mn0: 320.0,
        mn_l: 35.0,
        ms: 330.0,
        ad0: 54.0,
        ad_l: 3.2,
        ap0: 55.0,
        ap_l: 4.0,
        range: 5.6,
        atk_cd: 0.95,
        atk_style: 1,
        colour: [0.35, 0.9, 1.0],
        q: ab(
            "Micro Drones",
            "Release 3 homing drones at the targeted enemy, each biting for bonus damage.",
            [45.0, 50.0, 55.0],
            [7.0, 6.5, 6.0],
        ),
        w: ab(
            "Scan Beam",
            "Fire a piercing laser beam that burns and slows everything in the line.",
            [60.0, 65.0, 70.0],
            [12.0, 10.5, 9.0],
        ),
        e: ab(
            "Split",
            "Split from yourself: a hologram joins the fight for 5 seconds, striking for a share of your damage and taking none.",
            [50.0, 50.0, 50.0],
            [16.0, 14.0, 12.0],
        ),
        r: ab(
            "Hive Overload",
            "Split into four and every copy casts its beam and drones at the chosen enemy.",
            [100.0, 100.0, 100.0],
            [90.0, 75.0, 60.0],
        ),
    },
    ChampDef {
        key: "knight",
        name: "EmberKnight",
        title: "the flame duelist",
        hp0: 660.0,
        hp_l: 72.0,
        mn0: 260.0,
        mn_l: 28.0,
        ms: 340.0,
        ad0: 66.0,
        ad_l: 3.8,
        ap0: 30.0,
        ap_l: 2.0,
        range: 1.9,
        atk_cd: 0.85,
        atk_style: 3,
        colour: [1.0, 0.45, 0.12],
        q: ab(
            "Flame Tornado",
            "Call a tornado of fire at the cursor: it ticks damage and slows everything inside for a few seconds.",
            [55.0, 60.0, 65.0],
            [12.0, 12.0, 12.0],
        ),
        w: ab(
            "Immolate Guard",
            "Become immune to damage for 2 seconds, then carry a burning shield.",
            [50.0, 50.0, 50.0],
            [20.0, 18.0, 16.0],
        ),
        e: ab(
            "Ember Blade",
            "Set the sword in flames for 4 seconds: your attacks burn for extra damage.",
            [45.0, 45.0, 45.0],
            [15.0, 14.0, 13.0],
        ),
        r: ab(
            "Demon Form",
            "Become a flame demon: all your damage is doubled, and re-casting blinks you to the cursor — 3 charges.",
            [90.0, 90.0, 90.0],
            [110.0, 95.0, 80.0],
        ),
    },
    ChampDef {
        key: "hallow",
        name: "The Hallow One",
        title: "the lingering saint",
        hp0: 600.0,
        hp_l: 62.0,
        mn0: 420.0,
        mn_l: 45.0,
        ms: 330.0,
        ad0: 50.0,
        ad_l: 2.6,
        ap0: 65.0,
        ap_l: 4.6,
        range: 4.6,
        atk_cd: 1.0,
        atk_style: 2,
        colour: [0.92, 0.88, 0.6],
        q: ab(
            "Mend",
            "Heal the ally nearest the cursor (yourself in a duel) for a generous amount of health.",
            [60.0, 70.0, 80.0],
            [10.0, 10.0, 10.0],
        ),
        w: ab(
            "Hymn of Pacing",
            "The ally nearest the cursor moves 35% faster for 3 seconds.",
            [50.0, 50.0, 50.0],
            [14.0, 12.0, 10.0],
        ),
        e: ab(
            "Aegis",
            "Cloak the ally nearest the cursor in a shield that absorbs incoming damage.",
            [55.0, 55.0, 55.0],
            [15.0, 14.0, 13.0],
        ),
        r: ab(
            "Second Breath",
            "Mark an ally for a few seconds: the blow that would kill them instead leaves them standing, restored.",
            [100.0, 100.0, 100.0],
            [120.0, 105.0, 90.0],
        ),
    },
    ChampDef {
        key: "maw",
        name: "Bog Maw",
        title: "the hungry fen",
        hp0: 740.0,
        hp_l: 85.0,
        mn0: 210.0,
        mn_l: 22.0,
        ms: 320.0,
        ad0: 64.0,
        ad_l: 4.0,
        ap0: 15.0,
        ap_l: 1.0,
        range: 1.8,
        atk_cd: 0.95,
        atk_style: 4,
        colour: [0.42, 0.7, 0.3],
        q: ab(
            "Bog Hook",
            "Cast a hook that drags the first enemy caught to your feet and roots it.",
            [55.0, 55.0, 55.0],
            [13.0, 11.5, 10.0],
        ),
        w: ab(
            "Fen Shroud",
            "A rotting aura eats nearby enemies for a few seconds, and you drink half of it as health.",
            [50.0, 50.0, 50.0],
            [16.0, 15.0, 14.0],
        ),
        e: ab(
            "Silt Lunge",
            "Lunge toward the cursor, splashing silt that damages and slows everything you land in.",
            [45.0, 45.0, 45.0],
            [12.0, 10.5, 9.0],
        ),
        r: ab(
            "Bogquake",
            "The fen convulses around you: heavy damage and a rooting quake, then a sickening slow.",
            [100.0, 100.0, 100.0],
            [110.0, 95.0, 80.0],
        ),
    },
    ChampDef {
        key: "tessera",
        name: "Tessera",
        title: "the Clockmaker",
        hp0: 550.0,
        hp_l: 58.0,
        mn0: 340.0,
        mn_l: 40.0,
        ms: 325.0,
        ad0: 52.0,
        ad_l: 2.8,
        ap0: 70.0,
        ap_l: 5.0,
        range: 5.2,
        atk_cd: 1.05,
        atk_style: 2,
        colour: [0.75, 0.6, 1.0],
        q: ab(
            "Gear Shot",
            "Fire a piercing bolt of clockwork that punches through one enemy.",
            [45.0, 45.0, 45.0],
            [7.0, 7.0, 7.0],
        ),
        w: ab(
            "Chrono Trap",
            "Plant a trap at the cursor: the first enemy to cross it is rooted and burst for damage. Three held at once.",
            [50.0, 55.0, 60.0],
            [15.0, 13.0, 11.0],
        ),
        e: ab(
            "Chrono Step",
            "Wind yourself forward a short step and catch a burst of speed.",
            [40.0, 40.0, 40.0],
            [11.0, 9.5, 8.0],
        ),
        r: ab(
            "Grand Mechanism",
            "A great clock unwinds at the cursor: enemies inside are rooted and wound down, then it detonates.",
            [100.0, 100.0, 100.0],
            [110.0, 95.0, 80.0],
        ),
    },
];

/// Summoner-spell ids, matching the wire's `d`/`f` numbering.
pub const SPELL_FLASH: u8 = 0;
pub const SPELL_HEAL: u8 = 1;
pub const SPELL_SMITE: u8 = 2;
pub const SPELL_EXHAUST: u8 = 3;

/// Summoner spells, one for D and one for F.
#[derive(Clone, Copy, Debug)]
pub struct SpellDef {
    pub key: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub cd: f32,
}

pub const SPELLS: [SpellDef; 4] = [
    SpellDef {
        key: "flash",
        name: "Flash",
        desc: "Teleport a short distance toward the cursor.",
        cd: 210.0,
    },
    SpellDef {
        key: "heal",
        name: "Heal",
        desc: "Restore health to yourself and nearby allies.",
        cd: 150.0,
    },
    SpellDef {
        key: "smite",
        name: "Smite",
        desc: "Heavy true damage to a nearby minion or court.",
        cd: 60.0,
    },
    SpellDef {
        key: "exhaust",
        name: "Exhaust",
        desc: "Weaken a nearby enemy: less damage, less speed.",
        cd: 150.0,
    },
];

/// The eight runes; a page is any three of them.
#[derive(Clone, Copy, Debug)]
pub struct RuneDef {
    pub key: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

pub const RUNES: [RuneDef; 8] = [
    RuneDef {
        key: "fury",
        name: "Fury",
        desc: "+7% attack speed.",
    },
    RuneDef {
        key: "vigor",
        name: "Vigor",
        desc: "+70 health.",
    },
    RuneDef {
        key: "focus",
        name: "Focus",
        desc: "+14 ability power.",
    },
    RuneDef {
        key: "swift",
        name: "Swift",
        desc: "+5% move speed.",
    },
    RuneDef {
        key: "riches",
        name: "Riches",
        desc: "+15% gold from everything.",
    },
    RuneDef {
        key: "haste",
        name: "Haste",
        desc: "+7% ability haste.",
    },
    RuneDef {
        key: "cruelty",
        name: "Cruelty",
        desc: "+5% critical strike chance, +6% critical damage.",
    },
    RuneDef {
        key: "ruin",
        name: "Ruin",
        desc: "+8% spell damage.",
    },
];

/// The flat stat block everything adds to. Percent fields are percentage
/// points (7.0 = +7%).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub hp: f32,
    pub mana: f32,
    pub ad: f32,
    pub ap: f32,
    pub ms: f32,
    pub aspd: f32,
    pub crit: f32,
    pub critd: f32,
    pub haste: f32,
    pub gold: f32,
    pub spell: f32,
    pub dr: f32,
    pub mregen: f32,
}

impl Stats {
    /// The zero block; every literal below says `..Stats::ZERO` for the rest.
    pub const ZERO: Self = Self {
        hp: 0.0,
        mana: 0.0,
        ad: 0.0,
        ap: 0.0,
        ms: 0.0,
        aspd: 0.0,
        crit: 0.0,
        critd: 0.0,
        haste: 0.0,
        gold: 0.0,
        spell: 0.0,
        dr: 0.0,
        mregen: 0.0,
    };

    /// Component-wise sum. Percent fields add as percentage points.
    pub fn add(&mut self, other: &Self) {
        self.hp += other.hp;
        self.mana += other.mana;
        self.ad += other.ad;
        self.ap += other.ap;
        self.ms += other.ms;
        self.aspd += other.aspd;
        self.crit += other.crit;
        self.critd += other.critd;
        self.haste += other.haste;
        self.gold += other.gold;
        self.spell += other.spell;
        self.dr += other.dr;
        self.mregen += other.mregen;
    }

    /// Clamp every field into its legal range after all bonuses are in.
    pub const fn clamp_odds(&mut self) {
        self.crit = self.crit.clamp(0.0, 90.0);
        self.critd = self.critd.max(150.0);
        self.dr = self.dr.clamp(0.0, 60.0);
        self.aspd = self.aspd.clamp(0.0, 300.0);
        self.haste = self.haste.clamp(0.0, 250.0);
    }
}

/// What an item gives.
#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub id: u16,
    pub key: &'static str,
    pub name: &'static str,
    pub cost: u32,
    pub tier: u8,
    pub desc: &'static str,
    pub flat: Stats,
    /// On-hit burn applied by the wielder's attacks (flat + ratio x AP).
    pub burn: f32,
    pub burn_ap: f32,
    /// Damage dealt by the wielder's abilities slows the target 20% for 1 s.
    pub spell_slow: bool,
    /// Charges for consumables (0 = not a consumable).
    pub charges: u8,
    pub heal: f32,
    pub restore_mana: f32,
}

/// ids 1..18; 0 in an inventory slot means empty.
pub const ITEMS: [ItemDef; 18] = [
    ItemDef {
        id: 1,
        key: "sword",
        name: "Rusted Shortsword",
        cost: 350,
        tier: 1,
        desc: "+18 damage.",
        flat: Stats {
            ad: 18.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 2,
        key: "crystal",
        name: "Focus Crystal",
        cost: 400,
        tier: 1,
        desc: "+28 ability power.",
        flat: Stats {
            ap: 28.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 3,
        key: "heart",
        name: "Heart Gem",
        cost: 400,
        tier: 1,
        desc: "+160 health.",
        flat: Stats {
            hp: 160.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 4,
        key: "boots",
        name: "Wanderer Boots",
        cost: 300,
        tier: 1,
        desc: "+8% move speed.",
        flat: Stats {
            ms: 8.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 5,
        key: "ring",
        name: "Charm Ring",
        cost: 450,
        tier: 1,
        desc: "+10% ability haste.",
        flat: Stats {
            haste: 10.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 6,
        key: "coin",
        name: "Lucky Coin",
        cost: 400,
        tier: 1,
        desc: "+8% critical strike chance, +10% critical damage.",
        flat: Stats {
            crit: 8.0,
            critd: 10.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 7,
        key: "dagger",
        name: "Hunter's Dagger",
        cost: 850,
        tier: 2,
        desc: "+25 damage, +5% move speed.",
        flat: Stats {
            ad: 25.0,
            ms: 5.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 8,
        key: "font",
        name: "Mana Font",
        cost: 800,
        tier: 2,
        desc: "+180 mana, +60% mana regeneration.",
        flat: Stats {
            mana: 180.0,
            mregen: 60.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 9,
        key: "windstep",
        name: "Windstep",
        cost: 1150,
        tier: 2,
        desc: "+10% move speed, +5% ability haste.",
        flat: Stats {
            ms: 10.0,
            haste: 5.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 10,
        key: "orb",
        name: "Lifebound Orb",
        cost: 1300,
        tier: 2,
        desc: "+200 health, +30 ability power, +25% mana regeneration.",
        flat: Stats {
            hp: 200.0,
            ap: 30.0,
            mregen: 25.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 11,
        key: "emberbrand",
        name: "Emberbrand",
        cost: 2200,
        tier: 3,
        desc: "+42 damage. Your attacks burn for 14 (+0.25 AP) over 2 seconds.",
        flat: Stats {
            ad: 42.0,
            ..Stats::ZERO
        },
        burn: 14.0,
        burn_ap: 0.25,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 12,
        key: "storm",
        name: "Storm Choir",
        cost: 2500,
        tier: 3,
        desc: "+70 ability power, +15% ability haste.",
        flat: Stats {
            ap: 70.0,
            haste: 15.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 13,
        key: "duskveil",
        name: "Duskveil",
        cost: 2350,
        tier: 3,
        desc: "+45 ability power, +160 health. Your abilities slow by 20% for 1 second.",
        flat: Stats {
            ap: 45.0,
            hp: 160.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: true,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 14,
        key: "sunspear",
        name: "Sunspear",
        cost: 2700,
        tier: 3,
        desc: "+55 damage, +15% critical strike chance, +20% critical damage.",
        flat: Stats {
            ad: 55.0,
            crit: 15.0,
            critd: 20.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 15,
        key: "aegis",
        name: "Aegis Plate",
        cost: 2400,
        tier: 3,
        desc: "+350 health. You take 8% less damage.",
        flat: Stats {
            hp: 350.0,
            dr: 8.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 16,
        key: "ruin",
        name: "Gauntlets of Ruin",
        cost: 2600,
        tier: 3,
        desc: "+55 ability power, +10% spell damage.",
        flat: Stats {
            ap: 55.0,
            spell: 10.0,
            ..Stats::ZERO
        },
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 0,
        heal: 0.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 17,
        key: "hpotion",
        name: "Health Potion",
        cost: 50,
        tier: 1,
        desc: "Consumable, 5 charges: restore 110 health over 6 seconds (drink with key 1-6).",
        flat: Stats::ZERO,
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 5,
        heal: 110.0,
        restore_mana: 0.0,
    },
    ItemDef {
        id: 18,
        key: "mpotion",
        name: "Mana Potion",
        cost: 75,
        tier: 1,
        desc: "Consumable, 4 charges: restore 140 mana over 6 seconds (drink with key 1-6).",
        flat: Stats::ZERO,
        burn: 0.0,
        burn_ap: 0.0,
        spell_slow: false,
        charges: 4,
        heal: 0.0,
        restore_mana: 140.0,
    },
];

/// Look an item id up; 0 and unknown are "nothing".
pub fn item(id: u16) -> Option<&'static ItemDef> {
    ITEMS.iter().find(|i| i.id == id)
}

/// A champion's natural stat block at `level`, before runes and items.
#[must_use]
pub fn champ_stats(def: u8, level: u8) -> Stats {
    let c = &CHAMPS[usize::from(def.min(4))];
    let lv = f32::from(level.saturating_sub(1));
    Stats {
        hp: c.hp0 + c.hp_l * lv,
        mana: c.mn0 + c.mn_l * lv,
        ad: c.ad0 + c.ad_l * lv,
        ap: c.ap0 + c.ap_l * lv,
        ms: c.ms,
        aspd: 0.0,
        crit: 0.0,
        critd: 175.0,
        haste: 0.0,
        gold: 0.0,
        spell: 0.0,
        dr: 0.0,
        mregen: 0.0,
    }
}

/// The rune bonuses of a page (three indices, 255 = none).
#[must_use]
pub fn rune_stats(runes: &[u8; 3]) -> Stats {
    let mut s = Stats::ZERO;
    for &r in runes {
        match usize::from(r) {
            0 => s.aspd += 7.0,
            1 => s.hp += 70.0,
            2 => s.ap += 14.0,
            3 => s.ms += 5.0,
            4 => s.gold += 15.0,
            5 => s.haste += 7.0,
            6 => {
                s.crit += 5.0;
                s.critd += 6.0;
            }
            7 => s.spell += 8.0,
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_ids_are_unique_and_start_at_one() {
        let mut seen = 0u64;
        for i in &ITEMS {
            assert_eq!(u64::from(i.id), seen + 1, "ids must be dense from 1");
            seen = u64::from(i.id);
        }
    }

    #[test]
    fn ability_tables_are_per_rank() {
        for c in &CHAMPS {
            for a in [&c.q, &c.w, &c.e, &c.r] {
                assert_eq!(a.mana.len(), 3);
                assert_eq!(a.cd.len(), 3);
                for rank in 0..3 {
                    assert!(a.mana[rank] > 0.0, "{} {} mana", c.name, a.name);
                    assert!(a.cd[rank] > 0.0, "{} {} cd", c.name, a.name);
                }
            }
        }
    }

    #[test]
    fn xp_curve_rises() {
        assert!(xp_needed(1) < xp_needed(5));
        assert!(xp_needed(5) < xp_needed(MAX_LEVEL - 1));
    }

    #[test]
    fn champ_stats_grow() {
        let a = champ_stats(SWARM, 1);
        let b = champ_stats(SWARM, 5);
        assert!(b.hp > a.hp && b.ad > a.ad && b.ap > a.ap);
        assert!((a.ms - b.ms).abs() < 1e-6);
    }
}
