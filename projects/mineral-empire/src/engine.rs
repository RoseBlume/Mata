//! Incremental mining economy with unbounded-range currency and varied research.
use crate::big::Big;
use std::str::FromStr;
pub const COUNT:usize=60;
pub const PRESTIGE_COUNT:usize=14;
pub const RESEARCH_COUNT:usize=13;
pub const AUTO_RESEARCH_KIND:usize=8;
/// Ore access after reaching Tier 2. Tier 1 intentionally retains its old
/// unrestricted economy. Higher tiers progressively open deeper deposits.
/// Index 0 is the initial / Tier-1 economy; it is deliberately unchanged.
pub const LAST_ORE_BY_TIER:[usize;PRESTIGE_COUNT]=[
    COUNT-1,10,17,23,27,30,35,40,45,49,53,56,58,59,
];
/// Let output approach the *next* prestige threshold before gently damping it.
/// The old knee was seven or more decimal orders early, making several tiers
/// feel stalled even after a major upgrade. The thresholds and ore gates remain.
const SOFT_KNEE_START_DIGITS:u64=3;
const SOFT_OUTPUT_ROOT_START:u32=6;
const SOFT_OUTPUT_ROOT_STEP:u32=2;
/// Automation considers income-producing investments once per minute of play.
pub const AUTO_PURCHASE_INTERVAL:f64=60.0;
const AUTO_PURCHASE_LIMIT:usize=128;
/// Prefer upgrades that repay their cost within an hour, but allow small
/// opportunistic purchases when the treasury vastly exceeds their cost.
const AUTO_MAX_PAYBACK_SECONDS:f64=3600.0;
const AUTO_MAX_CASH_FRACTION_FOR_LONG_PAYBACK:f64=0.05;
pub const RESEARCH:[&str;RESEARCH_COUNT] = ["Extraction","Logistics","Automation","Auto-Gear","Refining","Prospecting","Quantum Drill","Auto-Prestige","Auto-Research","Specialization","Ore Mastery","Infrastructure","Legacy Resonance"];
#[derive(Clone,Copy)]
pub struct Mineral { pub name:&'static str,pub short:&'static str,pub base_cost:&'static str,pub revenue:&'static str,pub color:u32,pub hint:&'static str }
pub const ORES:[Mineral;COUNT]=[
    Mineral{name:"Coal",short:"COAL",base_cost:"10",revenue:"1.5",color:0xff8492a8,hint:"Fuel the first furnaces"},
    Mineral{name:"Iron",short:"IRON",base_cost:"150",revenue:"22",color:0xffce8467,hint:"Forge industrial tools"},
    Mineral{name:"Copper",short:"CU",base_cost:"3000",revenue:"460",color:0xffea9863,hint:"Wire a connected empire"},
    Mineral{name:"Silver",short:"AG",base_cost:"70000",revenue:"11500",color:0xffb9d4e8,hint:"Extract valuable seams"},
    Mineral{name:"Gold",short:"AU",base_cost:"2e6",revenue:"3.6e5",color:0xffffcb57,hint:"Control the gold rush"},
    Mineral{name:"Platinum",short:"PT",base_cost:"1e8",revenue:"2e7",color:0xff9fe5d9,hint:"Refine noble metals"},
    Mineral{name:"Diamond",short:"DIA",base_cost:"7e9",revenue:"1.6e9",color:0xff7dd3ff,hint:"Cut the deepest gems"},
    Mineral{name:"Emerald",short:"EM",base_cost:"9e11",revenue:"2.3e11",color:0xff59f5b8,hint:"Discover ancient caverns"},
    Mineral{name:"Uranium",short:"U",base_cost:"1.5e14",revenue:"4.5e13",color:0xffaeef6f,hint:"Supply frontier reactors"},
    Mineral{name:"Mythril",short:"MY",base_cost:"6e16",revenue:"2e16",color:0xffbb9cff,hint:"Mine the impossible"},
    Mineral{name:"Obsidian",short:"OBS",base_cost:"2e20",revenue:"6e19",color:0xff816dbe,hint:"Explore volcanic glass fields"},
    Mineral{name:"Titanium",short:"TI",base_cost:"4e24",revenue:"1e24",color:0xffa4c6da,hint:"Build ultra-light mine rigs"},
    Mineral{name:"Cobalt",short:"CO",base_cost:"8e28",revenue:"2e28",color:0xff76a9d8,hint:"Forge heat-resistant alloys"},
    Mineral{name:"Lithium",short:"LI",base_cost:"2e34",revenue:"5e33",color:0xfff5b0c2,hint:"Power a battery-powered world"},
    Mineral{name:"Palladium",short:"PD",base_cost:"6e40",revenue:"2e40",color:0xffd4bda6,hint:"Refine catalytic veins"},
    Mineral{name:"Rhodium",short:"RH",base_cost:"2e47",revenue:"7e46",color:0xffb4d4ed,hint:"Harvest reflective metal seams"},
    Mineral{name:"Iridium",short:"IR",base_cost:"9e54",revenue:"3e54",color:0xfff6c58b,hint:"Recover rare impact deposits"},
    Mineral{name:"Osmium",short:"OS",base_cost:"5e63",revenue:"2e63",color:0xff89b5d6,hint:"Mine the densest ores"},
    Mineral{name:"Tungsten",short:"W",base_cost:"3e73",revenue:"1e73",color:0xff99a8be,hint:"Reinforce mega drills"},
    Mineral{name:"Neodymium",short:"ND",base_cost:"2e84",revenue:"8e83",color:0xffd197e1,hint:"Extract magnetic deposits"},
    Mineral{name:"Sapphire",short:"SAP",base_cost:"1e96",revenue:"4e95",color:0xff4878ef,hint:"Cut royal-blue crystals"},
    Mineral{name:"Ruby",short:"RUB",base_cost:"8e108",revenue:"3e108",color:0xfff26a86,hint:"Separate corundum veins"},
    Mineral{name:"Alexandrite",short:"ALX",base_cost:"7e122",revenue:"2.6e122",color:0xff8de6b7,hint:"Discover color-shifting gems"},
    Mineral{name:"Tanzanite",short:"TZ",base_cost:"7e137",revenue:"3e137",color:0xff9d9bfa,hint:"Explore violet crystal belts"},
    Mineral{name:"Benitoite",short:"BEN",base_cost:"8e153",revenue:"3e153",color:0xff6dc4f2,hint:"Harvest fluorescent gemstones"},
    Mineral{name:"Painite",short:"PAI",base_cost:"1e172",revenue:"4e171",color:0xffe08589,hint:"Recover ultra-rare crystals"},
    Mineral{name:"Astatine",short:"AT",base_cost:"4e194",revenue:"1e194",color:0xffa9ef78,hint:"Isolate fleeting exotic elements"},
    Mineral{name:"Star Metal",short:"STAR",base_cost:"2e225",revenue:"7e224",color:0xffe7bf8e,hint:"Forge fictional stellar alloys"},
    Mineral{name:"Aetherite",short:"AETH",base_cost:"8e280",revenue:"3e280",color:0xff9b8df6,hint:"Excavate fictional aether seams"},
    Mineral{name:"Void Crystal",short:"VOID",base_cost:"1e350",revenue:"4e349",color:0xffe6a3f7,hint:"Uncover fictional void matter"},
    Mineral{name:"Bauxite",short:"BAUX",base_cost:"1e410",revenue:"3e409",color:0xffc77f5d,hint:"Strip-mine aluminum-rich deposits"},
    Mineral{name:"Chromite",short:"CR",base_cost:"1e440",revenue:"4e439",color:0xff4a6979,hint:"Extract corrosion-proof ore"},
    Mineral{name:"Molybdenite",short:"MO",base_cost:"1e475",revenue:"4e474",color:0xffa7adc2,hint:"Build hardened excavation tools"},
    Mineral{name:"Vanadinite",short:"VAN",base_cost:"1e515",revenue:"4e514",color:0xffd37b56,hint:"Harvest crimson ore crystals"},
    Mineral{name:"Zircon",short:"ZIR",base_cost:"1e560",revenue:"4e559",color:0xff8ecdc5,hint:"Mine ancient transparent gems"},
    Mineral{name:"Spodumene",short:"SPOD",base_cost:"1e610",revenue:"4e609",color:0xffcf92cb,hint:"Find lithium silicate seams"},
    Mineral{name:"Monazite",short:"MON",base_cost:"1e670",revenue:"4e669",color:0xffd9b06a,hint:"Harvest rare-earth sands"},
    Mineral{name:"Columbite",short:"COL",base_cost:"1e735",revenue:"4e734",color:0xff7d89a6,hint:"Recover niobium-rich veins"},
    Mineral{name:"Tantalite",short:"TAN",base_cost:"1e805",revenue:"4e804",color:0xff958c9b,hint:"Recover deep tantalum seams"},
    Mineral{name:"Scheelite",short:"SCH",base_cost:"1e880",revenue:"4e879",color:0xffecb16b,hint:"Illuminate tungsten deposits"},
    Mineral{name:"Peridot",short:"PER",base_cost:"1e960",revenue:"4e959",color:0xffa9df68,hint:"Cut olive-green gemstones"},
    Mineral{name:"Aquamarine",short:"AQUA",base_cost:"1e1050",revenue:"4e1049",color:0xff88ddea,hint:"Survey blue crystal tunnels"},
    Mineral{name:"Opal",short:"OPAL",base_cost:"1e1150",revenue:"4e1149",color:0xffbbdce9,hint:"Separate iridescent pockets"},
    Mineral{name:"Jadeite",short:"JADE",base_cost:"1e1260",revenue:"4e1259",color:0xff76d7a5,hint:"Mine high-pressure jade"},
    Mineral{name:"Grandidierite",short:"GRAN",base_cost:"1e1380",revenue:"4e1379",color:0xff6cc1ba,hint:"Extract blue-green anomalies"},
    Mineral{name:"Jeremejevite",short:"JER",base_cost:"1e1510",revenue:"4e1509",color:0xffe2c1df,hint:"Search elusive crystal caves"},
    Mineral{name:"Musgravite",short:"MUS",base_cost:"1e1650",revenue:"4e1649",color:0xffa99dd7,hint:"Refine high-value gem clusters"},
    Mineral{name:"Taaffeite",short:"TAAF",base_cost:"1e1800",revenue:"4e1799",color:0xffc69dc8,hint:"Survey the rarest formations"},
    Mineral{name:"Serendibite",short:"SER",base_cost:"1e1960",revenue:"4e1959",color:0xff538b87,hint:"Drill metamorphic formations"},
    Mineral{name:"Red Beryl",short:"RBER",base_cost:"1e2130",revenue:"4e2129",color:0xffef6b85,hint:"Recover scarlet mineral veins"},
    Mineral{name:"Hibonite",short:"HIB",base_cost:"1e2310",revenue:"4e2309",color:0xffab749f,hint:"Separate primordial minerals"},
    Mineral{name:"Eosphorite",short:"EOS",base_cost:"1e2500",revenue:"4e2499",color:0xffd6a9e9,hint:"Explore violet ore clusters"},
    Mineral{name:"Starlight Ore",short:"LIGHT",base_cost:"1e2700",revenue:"4e2699",color:0xffe9da97,hint:"Extract fictional luminous ore"},
    Mineral{name:"Solarite",short:"SOL",base_cost:"1e2920",revenue:"4e2919",color:0xffffb55f,hint:"Power solar foundries"},
    Mineral{name:"Nebulium",short:"NEB",base_cost:"1e3150",revenue:"4e3149",color:0xff8eabeb,hint:"Refine fictional nebula crystals"},
    Mineral{name:"Quasar Shard",short:"QSR",base_cost:"1e3390",revenue:"4e3389",color:0xffa7f0ef,hint:"Harvest stellar jet fragments"},
    Mineral{name:"Chronite",short:"TIME",base_cost:"1e3640",revenue:"4e3639",color:0xffc6aeff,hint:"Extract fictional temporal veins"},
    Mineral{name:"Riftstone",short:"RIFT",base_cost:"1e3900",revenue:"4e3899",color:0xffed9adb,hint:"Mine dimensional fractures"},
    Mineral{name:"Singularity Glass",short:"SG",base_cost:"1e4180",revenue:"4e4179",color:0xff91f2cd,hint:"Shape impossible pressure glass"},
    Mineral{name:"Eternium",short:"ETERN",base_cost:"1e4500",revenue:"4e4499",color:0xffffdd9e,hint:"Uncover the eternal motherlode"},
];
#[derive(Clone,Copy,Debug)]
pub struct Prestige {pub name:&'static str,pub currency:&'static str,pub threshold:&'static str,pub accent:u32,pub description:&'static str}
pub const STAGES:[Prestige;PRESTIGE_COUNT]=[
    Prestige{name:"Land Claims",currency:"Claims",threshold:"1e6",accent:0xffe7c278,description:"Survey new territories. Reset mines for permanent claims."},
    Prestige{name:"Industrial Forge",currency:"Forge Cores",threshold:"1e10",accent:0xfff89c7a,description:"Smelt industrial cores. Reset mines and land claims."},
    Prestige{name:"Mining Conglomerate",currency:"Shares",threshold:"1e23",accent:0xff7ad5e8,description:"Float your empire. Reset operations and forge cores."},
    Prestige{name:"Star Excavation",currency:"Relics",threshold:"1e65",accent:0xffba9dff,description:"Excavate other worlds. Rebuild the entire empire."},
    Prestige{name:"Deep Core",currency:"Core Fragments",threshold:"1e150",accent:0xff7fd7c0,description:"Break into the mantle. Restart previous legacies."},
    Prestige{name:"Continental Union",currency:"Union Seals",threshold:"1e330",accent:0xffd6b5f0,description:"Unite mining regions across continents."},
    Prestige{name:"Oceanic Extraction",currency:"Abyss Pearls",threshold:"1e710",accent:0xff7ccef2,description:"Harness seabed mining technologies."},
    Prestige{name:"Lunar Syndicate",currency:"Moon Crests",threshold:"1e1510",accent:0xffd4d7ee,description:"Control the first lunar extraction guild."},
    Prestige{name:"Asteroid Cartel",currency:"Asteroid Bonds",threshold:"1e3180",accent:0xfff0b781,description:"Harvest mineral-rich asteroid fields."},
    Prestige{name:"Nebula Foundry",currency:"Nebula Embers",threshold:"1e6700",accent:0xffd696e7,description:"Smelt matter inside a stellar nebula."},
    Prestige{name:"Galactic Combine",currency:"Galaxy Marks",threshold:"1e14100",accent:0xff9fe0dd,description:"Merge mining empires across galaxies."},
    Prestige{name:"Quantum Rift",currency:"Rift Keys",threshold:"1e29600",accent:0xffe795bb,description:"Open speculative quantum drilling gates."},
    Prestige{name:"Dimensional Dominion",currency:"Realm Sigils",threshold:"1e62100",accent:0xffb7aaff,description:"Build foundries across fictional realities."},
    Prestige{name:"Eternal Excavation",currency:"Eternity Shards",threshold:"1e130000",accent:0xffffdc89,description:"Mine beyond the final known horizon."},
];
/// Four distinctive prestige-dependent research paths instead of a repeated catalog.
/// These names belong to gameplay data, not just the UI.
pub const SPECIAL_RESEARCH_NAMES:[[&str;4];PRESTIGE_COUNT]=[
 ["Claim Surveyors","Coal Seam Mapping","Cheaper Contractors","Survey Records"],
 ["Foundry Catalysts","Alloy Prospecting","Recycled Tooling","Forge Blueprints"],
 ["Stock Dividends","Industrial Acquisitions","Bulk Procurement","Shareholder Trust"],
 ["Stellar Refineries","Meteorite Survey","Zero-G Drills","Astral Archives"],
 ["Mantle Furnaces","Volcanic Deposits","Heat Exchange","Core Memory"],
 ["Continental Rail","Borderless Mining","Supply Agreements","Union Charter"],
 ["Hydrothermal Power","Abyssal Ore","Pressure-Hardened Tools","Ocean Census"],
 ["Moonlight Smelting","Regolith Extraction","Vacuum Conveyors","Lunar Records"],
 ["Impact Reactors","Asteroid Lodes","Orbital Salvage","Prospector Guild"],
 ["Nebula Condensers","Gas Cloud Crystals","Ionized Drills","Foundry Echoes"],
 ["Galactic Logistics","Sector Extraction","Hyperlane Contracts","Interstellar Heritage"],
 ["Probability Engines","Unstable Rift Ore","Phase Shift Machinery","Quantum Memory"],
 ["Multiverse Foundries","Parallel Deposits","Dimension Gate Tools","Realm Histories"],
 ["Endless Excavators","Infinite Vein Tuning","Eternal Machinery","Age-Old Legacy"],
];
pub const SPECIAL_RESEARCH_DESCRIPTIONS:[&str;4]=[
 "Signature output multiplier unique to this prestige tier",
 "Strengthens a rotating class of mineral deposits",
 "Improves mining contracts, gear prices or drill efficiency",
 "Multiplies production using this tier's lifetime prestige",
];
/// One-time utility unlocks cannot waste x100 purchases on duplicate ranks.
/// Auto-Prestige reaches its minimum three-second cooldown at rank nine.
pub const fn research_rank_limit(kind:usize)->u32 {
    match kind {2|3|AUTO_RESEARCH_KIND=>1,7=>9,_=>u32::MAX}
}
pub fn research_name(stage:usize,kind:usize)->&'static str {
    if stage<PRESTIGE_COUNT && (9..13).contains(&kind){SPECIAL_RESEARCH_NAMES[stage][kind-9]}
    else{RESEARCH.get(kind).copied().unwrap_or("Unknown Research")}
}
pub fn research_description(_stage:usize,kind:usize)->&'static str {
    match kind {
        0=>"Extraction: x1.30 per rank (diminishing returns after rank 12)",
        1=>"Logistics: x1.14 per rank (diminishing returns after rank 12)",
        2=>"Optimizes miner purchases every 60 seconds (one rank required)",
        3=>"Optimizes equipment purchases every 60 seconds (one rank required)",
        4=>"Refining: x1.52 per rank (diminishing returns after rank 12)",
        5=>"Prospecting: reduces hiring and gear costs",
        6=>"Quantum drilling: x1.25 per rank (diminishing returns after rank 12)",
        7=>"Automates this tier's prestige resets; reduces delay",
        8=>"Unlocks automatic research purchases at all tiers",
        9..=10=>SPECIAL_RESEARCH_DESCRIPTIONS[kind-9],
        11=>match _stage%3 {
            0=>"Cheaper mine hiring contracts (8% per rank)",
            1=>"Cheaper equipment purchases (9% per rank)",
            _=>"Gear amplification (x1.07 per rank per tech level)",
        },
        12=>SPECIAL_RESEARCH_DESCRIPTIONS[3],
        _=>"",
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ResearchAmount{One,Ten,Fifty,Hundred}
impl ResearchAmount {
    pub const ALL:[Self;4]=[Self::One,Self::Ten,Self::Fifty,Self::Hundred];
    pub const fn count(self)->u32{match self{Self::One=>1,Self::Ten=>10,Self::Fifty=>50,Self::Hundred=>100}}
    pub const fn label(self)->&'static str{match self{Self::One=>"x1",Self::Ten=>"x10",Self::Fifty=>"x50",Self::Hundred=>"x100"}}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum BuyAmount{One,Ten,Max}
impl BuyAmount{pub fn name(self)->&'static str{match self{Self::One=>"x1",Self::Ten=>"x10",Self::Max=>"MAX"}}}
#[derive(Clone)]
pub struct Game {
    pub cash:Big,pub run_earned:Big,pub all_time_earned:Big,pub total_mined:[Big;COUNT],
    pub miners:[u64;COUNT],pub equipment:[u32;COUNT],pub prestige:[Big;PRESTIGE_COUNT],
    pub prestige_lifetime:[Big;PRESTIGE_COUNT],pub research:[[u32;RESEARCH_COUNT];PRESTIGE_COUNT],
    pub auto_buy:bool,pub auto_gear:bool,pub auto_prestige:bool,pub auto_research:bool,
    /// Minimum reward for each automatic ascension, without an f64 exponent cap.
    pub auto_prestige_min_gain:Big,
    pub auto_prestige_elapsed:f64,
    /// Seconds accumulated toward the shared 60-second hire/gear decision.
    pub auto_economy_elapsed:f64,
    pub achievements:[bool;8],pub elapsed_seconds:f64,pub resets:[u64;PRESTIGE_COUNT],
}
impl Default for Game{fn default()->Self{Self{
    cash:Big::from(25.),run_earned:Big::zero(),all_time_earned:Big::zero(),
    total_mined:std::array::from_fn(|_|Big::zero()),miners:[0;COUNT],equipment:[0;COUNT],
    prestige:std::array::from_fn(|_|Big::zero()),prestige_lifetime:std::array::from_fn(|_|Big::zero()),
    research:[[0;RESEARCH_COUNT];PRESTIGE_COUNT],auto_buy:false,auto_gear:false,
    auto_prestige:false,auto_research:false,auto_prestige_min_gain:Big::one(),auto_prestige_elapsed:0.,auto_economy_elapsed:0.,
    achievements:[false;8],
    elapsed_seconds:0.,resets:[0;PRESTIGE_COUNT],
}}}
fn n(text:&str)->Big {Big::from_str(text).unwrap_or_else(|_|Big::from(text.parse::<f64>().unwrap_or(0.)))}
/// Diminishing research returns after rank 12. Every doubling of ranks
/// beyond that point contributes roughly one more effective rank. This
/// prevents a large prestige balance plus x100 / auto-research from creating
/// thousands of full-strength exponential bonuses in a few seconds.
fn effective_research_ranks(rank:u32)->u64 {
    const FULL_STRENGTH:u32=12;
    if rank<=FULL_STRENGTH {return rank as u64}
    let excess=rank-FULL_STRENGTH;
    FULL_STRENGTH as u64 + (u32::BITS-excess.leading_zeros()) as u64
}

/// High-tier prestige rewards retain diminishing returns, but reach the
/// repeat-reset prerequisites in fewer runs than the previous rebalance.
fn prestige_reward_ratio(stage:usize)->(u32,u32) {
    // Tier 1 rewards are left alone. Starting at Tier 2, enormous run
    // overshoots no longer mint exponentially explosive prestige balances.
    match stage {
        0 => (7,25),
        1 => (1,7),
        _ => (1,9+2*(stage as u32-2)),
    }
}

#[derive(Clone)]
struct RateInputs {
    global:Big,
    quantum:Big,
    diversification:f64,
    gear_bonus:Big,
    mastery:[Big;7],
    brake:Option<(Big,u32)>,
}

impl Game{
pub fn base_cost(&self,i:usize)->Big{n(ORES[i].base_cost)}
pub fn unlock_cost(&self,i:usize)->Big{self.base_cost(i).mul_f64(0.65)}
fn ore_growth(&self,i:usize)->f64{
    let base=match i%4{0=>1.32,1=>1.36,2=>1.29,_=>1.41};
    let tier=self.current_prestige_tier();
    if tier==0 {base} else {base*(1.+0.02*tier as f64)}
}
fn gear_growth(&self,i:usize)->f64{
    let base=match i%4{0=>4.0,1=>3.7,2=>4.4,_=>4.6};
    let tier=self.current_prestige_tier();
    if tier==0 {base} else {base*(1.+0.05*tier as f64)}
}
fn gear_power(&self,i:usize)->f64{match i%4{0=>1.8,1=>2.05,2=>1.7,_=>2.3}}
fn total_research(&self,kind:usize)->u64{self.research.iter().map(|r|r[kind] as u64).sum()}
/// Existing unlocks are still owned and saved. Only the size of production,
/// research and price multipliers diminishes from Tier 2 onward.
fn effective_tier_research_ranks(&self,rank:u32)->u64{
    if self.current_prestige_tier()==0{return effective_research_ranks(rank)}
    if rank<=6{return rank as u64}
    6+(u32::BITS-(rank-6).leading_zeros()) as u64
}
fn total_effective_research(&self,kind:usize)->u64{
    self.research.iter().map(|r|self.effective_tier_research_ranks(r[kind])).sum()
}
/// Highest ore allowed by the presently attained prestige stage. The
/// first prestige stage's access is not restricted by this rebalance.
pub fn highest_accessible_ore(&self)->usize{
    LAST_ORE_BY_TIER[self.current_prestige_tier()]
}
/// Tier number (one based) at which an ore reopens after Tier 1.
pub fn ore_required_tier(ore:usize)->usize{
    LAST_ORE_BY_TIER.iter().enumerate().skip(1)
        .find(|(_,last)|ore<=**last).map(|(tier,_)|tier+1)
        .unwrap_or(PRESTIGE_COUNT)
}
pub fn ore_tier_available(&self,ore:usize)->bool{
    ore<COUNT && ore<=self.highest_accessible_ore()
}
/// Prepare the knee once per tick; do not reparse the requirement for
/// every ore and every UI rate calculation.
fn progression_brake_parameters(&self)->Option<(Big,u32)>{
    let tier=self.current_prestige_tier();
    if tier==0 || tier+1>=PRESTIGE_COUNT{return None}
    let digits=SOFT_KNEE_START_DIGITS+u64::from(tier>1);
    let knee=n(STAGES[tier+1].threshold)
        .div(&Big::from(10.).pow_u64(digits));
    let root=SOFT_OUTPUT_ROOT_START+
        SOFT_OUTPUT_ROOT_STEP*(tier as u32-1);
    Some((knee,root))
}
/// Compresses *income*, not saved cash. The fractional-power tail is
/// monotonic and unbounded; the Big exponent never touches f64.
fn apply_progression_brake(raw:Big,params:Option<&(Big,u32)>)->Big{
    let Some((knee,root))=params else{return raw};
    if raw<=*knee{return raw}
    if *root==SOFT_OUTPUT_ROOT_START {
        // Tier 2 only: a gentle logarithmic tail protects old saves with
        // extreme output without stalling ordinary production below the knee.
        let orders=raw.div(knee).log10_approx();
        return knee.mul(&Big::one().add(&orders).pow_u64(2));
    }
    knee.mul(&raw.div(knee).pow_ratio(1,*root))
}
#[cfg(test)]
fn progression_brake(&self,raw:Big)->Big{
    Self::apply_progression_brake(raw,self.progression_brake_parameters().as_ref())
}
/// Prestige currency above 1,000 has diminishing production returns.
/// Small balances behave as before, while later tiers receive a softer
/// penalty than the previous progression rebalance.
fn softened_prestige_balance(&self,stage:usize)->Big {
    let balance=&self.prestige[stage];
    let pivot=Big::from(1_000.);
    let softened=if balance<=&pivot {balance.clone()} else {
        let denominator=3+(stage/4) as u32;
        pivot.mul(&balance.div(&pivot).pow_ratio(1,denominator))
    };
    // Tier 1 plays identically to the old economy. Later tiers cannot
    // endlessly compound trillion-fold bonuses from earlier currencies.
    let tier=self.current_prestige_tier();
    if tier==0 || softened<=Big::one(){return softened}
    softened.pow_ratio(1,2+tier as u32)
}
pub fn miner_cost(&self,ore:usize,amount:u64)->Big{
    if ore>=COUNT||amount==0{return Big::zero()}
    let growth=self.ore_growth(ore);
    let start=Big::from(growth).pow_u64(self.miners[ore]);
    let series=Big::from(growth).pow_u64(amount).sub(&Big::one()).div(&Big::from(growth-1.));
    let discount=Big::from(0.97).pow_u64(self.total_effective_research(5));
    let specialized_discount=(0..PRESTIGE_COUNT).filter(|&stage|stage%3==0)
        .fold(Big::one(),|value,stage|value.mul(&Big::from(0.92).pow_u64(self.effective_tier_research_ranks(self.research[stage][11]))));
    self.base_cost(ore).mul(&start).mul(&series).mul(&discount).mul(&specialized_discount)
}
pub fn max_affordable(&self,ore:usize)->u64{
    if ore>=COUNT||!self.unlocked(ore)||self.cash<self.miner_cost(ore,1){return 0}
    let max=1_000_000u64.min(u64::MAX-self.miners[ore]);if max==0{return 0;}
    let mut low=1u64;let mut high=2u64;
    while high<max&&self.miner_cost(ore,high)<=self.cash {low=high;high=(high*2).min(max)}
    if high==max&&self.miner_cost(ore,high)<=self.cash{return high}
    while low+1<high{let mid=low+(high-low)/2;if self.miner_cost(ore,mid)<=self.cash{low=mid}else{high=mid}}low
}
pub fn buy_miner(&mut self,ore:usize,amount:BuyAmount)->u64{
    if ore>=COUNT||!self.unlocked(ore){return 0}
    let affordable=self.max_affordable(ore);let count=match amount{BuyAmount::One=>1,BuyAmount::Ten=>10,BuyAmount::Max=>affordable}.min(affordable);
    if count==0{return 0}let cost=self.miner_cost(ore,count);self.cash=self.cash.sub(&cost).clamp_nonnegative();
    self.miners[ore]+=count;self.check_achievements();count
}
pub fn equipment_cost(&self,ore:usize)->Big{
    if ore>=COUNT{return Big::zero()}
    let discount=Big::from(0.96).pow_u64(self.total_effective_research(5));
    let specialized_discount=(0..PRESTIGE_COUNT).filter(|&stage|stage%3==1)
        .fold(Big::one(),|value,stage|value.mul(&Big::from(0.91).pow_u64(self.effective_tier_research_ranks(self.research[stage][11]))));
    self.base_cost(ore).mul_f64(18.).mul(&Big::from(self.gear_growth(ore)).pow_u64(self.equipment[ore] as u64)).mul(&discount).mul(&specialized_discount)
}
pub fn buy_equipment(&mut self,ore:usize)->bool{
    if ore>=COUNT||!self.ore_tier_available(ore)||self.miners[ore]==0||self.equipment[ore]==u32::MAX{return false}
    let price=self.equipment_cost(ore);if self.cash<price{return false}
    self.cash=self.cash.sub(&price).clamp_nonnegative();self.equipment[ore]+=1;true
}
pub fn unlocked(&self,ore:usize)->bool{
    self.ore_tier_available(ore)
        && (ore==0||self.run_earned>=self.unlock_cost(ore)||self.miners[ore]>0)
}
pub fn global_multiplier(&self)->Big{
    let mut value=Big::one();
    let factors=[0.10,0.42,1.0,2.0,3.0,4.5,6.5,9.5,15.0,24.0,38.0,60.0,100.0,180.0];
    for (stage,factor) in factors.into_iter().enumerate(){
        let balance=self.softened_prestige_balance(stage);
        value=value.mul(&Big::one().add(&balance.mul_f64(factor)));
        value=value.mul(&Big::from(1.30).pow_u64(self.effective_tier_research_ranks(self.research[stage][0])));
        value=value.mul(&Big::from(1.14).pow_u64(self.effective_tier_research_ranks(self.research[stage][1])));
        value=value.mul(&Big::from(1.52).pow_u64(self.effective_tier_research_ranks(self.research[stage][4])));
        // Signatures still differ by tier, but cannot grow at full strength
        // indefinitely from automatic bulk purchases.
        value=value.mul(&Big::from(1.22+stage as f64*0.025)
            .pow_u64(self.effective_tier_research_ranks(self.research[stage][9])));
        let resonance=self.research[stage][12];
        if resonance>0 && self.prestige_lifetime[stage].is_positive(){
            // Lifetime legacies need the same diminishing returns as balances:
            // otherwise an enormous previous-tier windfall defeats all gates.
            let lifetime=&self.prestige_lifetime[stage];
            let pivot=Big::from(1_000.);
            let softened=if lifetime<=&pivot {lifetime.clone()}
                else {pivot.mul(&lifetime.div(&pivot).pow_ratio(1,4))};
            let lifetime_power=softened.pow_ratio(1,3);
            value=value.mul(&Big::one().add(&lifetime_power
                .mul_f64(0.015*self.effective_tier_research_ranks(resonance) as f64)));
        }
    }
    value
}
/// All shared factors, including ore-class mastery, are built once per tick.
/// Avoid recalculating fourteen prestige stages for each of sixty minerals.
fn ore_rate_cached(&self,ore:usize,factors:&RateInputs)->Big{
    // Existing owned ores are retained across save migration, but they
    // cannot bypass the new tier progression until their era is reached.
    if ore>=COUNT||!self.ore_tier_available(ore)||self.miners[ore]==0{return Big::zero()}
    let gear=Big::from(self.gear_power(ore)).mul(&factors.gear_bonus)
        .pow_u64(self.equipment[ore] as u64);
    let raw=n(ORES[ore].revenue)
        .mul(&Big::from(self.miners[ore])).mul(&gear)
        .mul(&factors.mastery[ore%7])
        .mul(&factors.quantum).mul_f64(factors.diversification)
        .mul(&factors.global).mul_f64(0.28);
    Self::apply_progression_brake(raw,factors.brake.as_ref())
}
fn rate_inputs(&self)->RateInputs{
    let global=self.global_multiplier();
    let quantum=Big::from(1.25).pow_u64(self.total_effective_research(6));
    let diversification=1.+self.miners.iter().filter(|&&m|m>0).count() as f64*0.012;
    let gear_bonus=(0..PRESTIGE_COUNT).filter(|&stage|stage%3==2)
        .fold(Big::one(),|value,stage|value.mul(&Big::from(1.07).pow_u64(self.effective_tier_research_ranks(self.research[stage][11]))));
    let mastery=std::array::from_fn(|group|{
        (0..PRESTIGE_COUNT).filter(|&stage|stage%7==group)
            .fold(Big::one(),|value,stage|value.mul(&Big::from(1.45+stage as f64*0.02)
                .pow_u64(self.effective_tier_research_ranks(self.research[stage][10]))))
    });
    let brake=self.progression_brake_parameters();
    RateInputs{global,quantum,diversification,gear_bonus,mastery,brake}
}
pub fn ore_rate(&self,ore:usize)->Big{
    if ore>=COUNT || !self.ore_tier_available(ore) || self.miners[ore]==0{return Big::zero()}
    self.ore_rate_cached(ore,&self.rate_inputs())
}
pub fn total_rate(&self)->Big{
    let factors=self.rate_inputs();
    (0..COUNT).fold(Big::zero(),|acc,i|acc.add(&self.ore_rate_cached(i,&factors)))
}
pub fn tick(&mut self,seconds:f64){
    if !seconds.is_finite()||seconds<=0.{return}
    let seconds=seconds.min(86400.);
    let factors=self.rate_inputs();
    let mut earned=Big::zero();
    for i in 0..COUNT{
        let produced=self.ore_rate_cached(i,&factors).mul_f64(seconds);
        self.total_mined[i]=self.total_mined[i].add(&produced);
        earned=earned.add(&produced);
    }
    self.cash=self.cash.add(&earned);
    self.run_earned=self.run_earned.add(&earned);
    self.all_time_earned=self.all_time_earned.add(&earned);
    self.elapsed_seconds=(self.elapsed_seconds+seconds).min(1e15);
    self.auto_prestige_elapsed=(self.auto_prestige_elapsed+seconds).min(3600.);
    if (self.auto_buy&&self.auto_hire_unlocked()) || (self.auto_gear&&self.auto_gear_unlocked()) {
        self.auto_economy_elapsed=(self.auto_economy_elapsed+seconds).min(AUTO_PURCHASE_INTERVAL);
    }
    self.check_achievements();
}
pub fn auto_hire_unlocked(&self)->bool{self.total_research(2)>0}
pub fn auto_gear_unlocked(&self)->bool{self.total_research(3)>0}
pub fn auto_research_unlocked(&self)->bool{self.total_research(AUTO_RESEARCH_KIND)>0}
/// The current prestige tier is the highest stage the player has actually
/// ascended to (including the currency's lifetime history). Tier 0 is the
/// starting tier and never receives automatic resets before tier 1 is reached.
pub fn current_prestige_tier(&self)->usize{
    self.prestige_lifetime.iter().rposition(Big::is_positive).unwrap_or(0)
}
/// Auto-Prestige must be researched somewhere in the current progression.
/// The one unlock automates *all* lower tiers: it does not require owning a
/// distinct Auto-Prestige upgrade in each lower tier after they are reset.
pub fn auto_prestige_unlocked(&self)->bool{
    self.research.iter().any(|row|row[7]>0)
}
pub fn set_auto_prestige_min_gain(&mut self,text:&str)->Result<(),String>{
    let text=text.trim();
    if text.len()>256{return Err("Minimum gain is too long (256 characters max)".into())}
    let normalized=if text.contains('e') || text.contains('E'){text.to_owned()}else{format!("{text}e0")};
    let value=normalized.parse::<Big>()?;
    if !value.is_positive(){return Err("Minimum gain must be greater than zero".into())}
    self.auto_prestige_min_gain=value;
    Ok(())
}
pub fn auto_prestige_target_gain(&self)->&Big{&self.auto_prestige_min_gain}
/// Scan every tier *strictly below* the current tier. Attempt the highest
/// eligible stage first so cheap resets cannot starve higher progression.
/// A successful ascension clears the current run; the next one must wait
/// until the normal gain requirement and cooldown are met again.
pub fn auto_prestige_tick(&mut self)->Option<(usize,Big)>{
    if !self.auto_prestige || !self.auto_prestige_unlocked(){return None}
    let highest=self.current_prestige_tier();
    if highest==0{return None}
    let rank=self.research.iter().map(|row|row[7]).max().unwrap_or(0);
    let cooldown=(15.0-(rank.saturating_sub(1) as f64*1.5)).max(3.0);
    if self.auto_prestige_elapsed<cooldown{return None}
    for stage in (0..highest).rev(){
        if !self.prestige_available(stage){continue}
        let gain=self.prestige_gain(stage);
        if gain.is_zero() || gain<self.auto_prestige_min_gain{continue}
        let actual=self.ascend(stage);
        if !actual.is_zero(){return Some((stage,actual))}
    }
    None
}
/// Choose the best affordable marginal production gain per dollar, across
/// both hires and equipment. Use the same per-ore production formula as the UI,
/// including gear, softcaps and new-mine diversification, instead of comparing
/// unscaled ore revenue or approximating a gear upgrade by its percentage.
///
/// A single shared cycle means enabling both automations cannot double-spend
/// cash independently. Re-evaluate after each purchase, as costs and income
/// change; skip investments with very long paybacks unless they are a tiny
/// fraction of the treasury.
pub fn auto_tick(&mut self){
    let hire=self.auto_buy&&self.auto_hire_unlocked();
    let gear=self.auto_gear&&self.auto_gear_unlocked();
    if !(hire||gear) || self.auto_economy_elapsed<AUTO_PURCHASE_INTERVAL{return}
    self.auto_economy_elapsed=0.;
    for _ in 0..AUTO_PURCHASE_LIMIT {
        let factors=self.rate_inputs();
        let mut next_factors=factors.clone();
        next_factors.diversification+=0.012;
        let mut diversity_gain=Big::zero();
        if hire {
            // On hiring the first miner in a new shaft, diversification also
            // boosts every *existing* mine. This is the same softcapped gain
            // used by total_rate, but computed only once per selection pass.
            for ore in 0..=self.highest_accessible_ore(){
                if self.miners[ore]>0 {
                    let before=self.ore_rate_cached(ore,&factors);
                    let after=self.ore_rate_cached(ore,&next_factors);
                    diversity_gain=diversity_gain.add(&after.sub(&before).clamp_nonnegative());
                }
            }
        }
        // (ore, buy_gear, gain/cost): comparing dimensionless ROI is valid
        // even for Big values with enormous exponents.
        let mut best:Option<(usize,bool,Big)>=None;
        let reserve_cap=self.cash.mul_f64(AUTO_MAX_CASH_FRACTION_FOR_LONG_PAYBACK);
        for ore in 0..=self.highest_accessible_ore(){
            let before=self.ore_rate_cached(ore,&factors);
            if hire && self.unlocked(ore) && self.miners[ore]<u64::MAX {
                let cost=self.miner_cost(ore,1);
                if cost.is_positive() && cost<=self.cash {
                    self.miners[ore]+=1;
                    let after=if self.miners[ore]==1 {
                        self.ore_rate_cached(ore,&next_factors)
                    }else{self.ore_rate_cached(ore,&factors)};
                    self.miners[ore]-=1;
                    let mut gain=after.sub(&before).clamp_nonnegative();
                    if self.miners[ore]==0 {gain=gain.add(&diversity_gain)}
                    if gain.is_positive() && (cost<=gain.mul_f64(AUTO_MAX_PAYBACK_SECONDS) || cost<=reserve_cap){
                        let roi=gain.div(&cost);
                        if best.as_ref().is_none_or(|(_,_,old)|roi> *old){best=Some((ore,false,roi));}
                    }
                }
            }
            if gear && self.miners[ore]>0 && self.equipment[ore]<u32::MAX {
                let cost=self.equipment_cost(ore);
                if cost.is_positive() && cost<=self.cash {
                    self.equipment[ore]+=1;
                    let after=self.ore_rate_cached(ore,&factors);
                    self.equipment[ore]-=1;
                    let gain=after.sub(&before).clamp_nonnegative();
                    if gain.is_positive() && (cost<=gain.mul_f64(AUTO_MAX_PAYBACK_SECONDS) || cost<=reserve_cap){
                        let roi=gain.div(&cost);
                        if best.as_ref().is_none_or(|(_,_,old)|roi> *old){best=Some((ore,true,roi));}
                    }
                }
            }
        }
        let Some((ore,is_gear,_))=best else {break};
        if is_gear {
            if !self.buy_equipment(ore){break}
        }else if self.buy_miner(ore,BuyAmount::One)==0 {break}
    }
}
/// Purchase a bounded number of the most affordable researched improvements.
/// One tick cannot consume an unlimited balance or loop forever.
pub fn auto_research_tick(&mut self)->u32{
    if !self.auto_research || !self.auto_research_unlocked(){return 0}
    let mut bought=0;
    for _ in 0..8 {
        let mut cheapest:Option<(usize,usize,Big)>=None;
        for stage in 0..PRESTIGE_COUNT {
            if !self.prestige[stage].is_positive(){continue}
            for kind in 0..RESEARCH_COUNT {
                if self.research[stage][kind]>=research_rank_limit(kind){continue}
                let cost=self.research_cost(stage,kind);
                if cost>self.prestige[stage]{continue}
                if cheapest.as_ref().is_none_or(|(_,_,lowest)|cost<*lowest){cheapest=Some((stage,kind,cost));}
            }
        }
        let Some((stage,kind,_))=cheapest else{break};
        if self.buy_research_bulk(stage,kind,1)==0{break}
        bought+=1;
    }
    bought
}
/// The minimum lifetime currency in the immediately previous tier.
/// Tier 1-3 are unchanged; from Tier 4 onward it doubles with every tier.
/// A large run can fulfill the requirement at once, but it must have been
/// earned from the previous tier, not from an unrelated mine balance.
pub const fn prestige_prerequisite(stage:usize)->u64{
    match stage {
        0=>0,
        1=>6,
        2=>4,
        3..=13=>25u64 << (stage-3),
        _=>u64::MAX,
    }
}
pub fn prestige_available(&self,stage:usize)->bool{
    if stage>=PRESTIGE_COUNT{return false}
    if stage>0&&self.prestige_lifetime[stage-1]<Big::from(Self::prestige_prerequisite(stage)){return false}
    self.run_earned>=n(STAGES[stage].threshold)
}
pub fn prestige_gain(&self,stage:usize)->Big{
    if !self.prestige_available(stage){return Big::zero()}
    {
        let (numerator,denominator)=prestige_reward_ratio(stage);
        self.run_earned.div(&n(STAGES[stage].threshold))
            .pow_ratio(numerator,denominator).floor()
    }
}
pub fn ascend(&mut self,stage:usize)->Big{
    let gain=self.prestige_gain(stage);if gain.is_zero(){return Big::zero()}
    self.prestige[stage]=self.prestige[stage].add(&gain);self.prestige_lifetime[stage]=self.prestige_lifetime[stage].add(&gain);
    self.resets[stage]=self.resets[stage].saturating_add(1);
    for i in 0..stage {self.prestige[i]=Big::zero();self.research[i]=[0;RESEARCH_COUNT]}
    self.cash=Big::from(25.);self.run_earned=Big::zero();self.miners=[0;COUNT];self.equipment=[0;COUNT];
    // Automation toggles remain configured, but an upgrade must still be
    // owned before an automatic action runs after a higher-tier reset.
    // The purchasing clock is global, not per-run: frequent auto-prestige
    // resets must not starve auto-hiring indefinitely.
    self.auto_prestige_elapsed=0.;self.check_achievements();gain
}
/// Cost of one additional rank at a particular owned level.
fn research_price_at(&self,stage:usize,kind:usize,level:u32)->Big{
    if stage>=PRESTIGE_COUNT||kind>=RESEARCH_COUNT{return Big::zero()}
    let base=[1.,2.,4.,6.,8.,10.,18.,14.,22.,9.,15.,13.,25.][kind];
    let growth=if kind<=8 {3.1} else {2.7+0.05*(stage%5) as f64};
    let tier=self.current_prestige_tier();
    let growth=if tier==0 {growth} else {growth*(1.0+0.045*tier as f64)};
    Big::from(base*(1.0+stage as f64*0.07))
        .mul(&Big::from(growth).pow_u64(level as u64))
}
pub fn research_cost(&self,stage:usize,kind:usize)->Big{
    if stage>=PRESTIGE_COUNT||kind>=RESEARCH_COUNT{return Big::zero()}
    if self.research[stage][kind]>=research_rank_limit(kind){return Big::zero()}
    self.research_price_at(stage,kind,self.research[stage][kind])
}
/// Bulk cost is the exact sum of the same ranks as single purchases, not
/// `count * current_price` (which would make high quantities too cheap).
pub fn research_bulk_cost(&self,stage:usize,kind:usize,count:u32)->Big{
    if stage>=PRESTIGE_COUNT||kind>=RESEARCH_COUNT||count==0{return Big::zero()}
    let capped=count.min(research_rank_limit(kind).saturating_sub(self.research[stage][kind]));
    let mut total=Big::zero();
    for offset in 0..capped {
        total=total.add(&self.research_price_at(stage,kind,self.research[stage][kind]+offset));
    }
    total
}
/// Buys as many ranks as affordable, up to the selected amount, atomically.
/// In particular, x100 with money for 31 ranks buys 31 instead of failing.
pub fn buy_research_bulk(&mut self,stage:usize,kind:usize,count:u32)->u32{
    if stage>=PRESTIGE_COUNT||kind>=RESEARCH_COUNT{return 0}
    let capped=count.min(100).min(research_rank_limit(kind).saturating_sub(self.research[stage][kind]));
    let mut total=Big::zero();
    let mut bought=0;
    for offset in 0..capped {
        let next=total.add(&self.research_price_at(stage,kind,self.research[stage][kind]+offset));
        if next>self.prestige[stage]{break}
        total=next;bought+=1;
    }
    if bought>0{
        self.prestige[stage]=self.prestige[stage].sub(&total).clamp_nonnegative();
        self.research[stage][kind]+=bought;
    }
    bought
}
pub fn buy_research(&mut self,stage:usize,kind:usize)->bool{self.buy_research_bulk(stage,kind,1)>0}
pub fn check_achievements(&mut self){
    self.achievements[0]|=self.miners.iter().any(|&n|n>=10);
    self.achievements[1]|=self.all_time_earned>=n("1e6");
    self.achievements[2]|=self.miners.iter().filter(|&&v|v>0).count()>=5;
    self.achievements[3]|=self.prestige_lifetime[0]>=Big::one();
    self.achievements[4]|=self.prestige_lifetime[1]>=Big::one();
    self.achievements[5]|=self.prestige_lifetime[2]>=Big::one();
    self.achievements[6]|=self.prestige_lifetime[3]>=Big::one();
    self.achievements[7]|=self.total_mined[9]>=Big::one();
}
pub fn sanitize(&mut self){
    if self.cash<Big::zero(){self.cash=Big::zero()}
    if self.run_earned<Big::zero(){self.run_earned=Big::zero()}
    if self.all_time_earned<Big::zero(){self.all_time_earned=Big::zero()}
    if !self.elapsed_seconds.is_finite()||self.elapsed_seconds<0.{self.elapsed_seconds=0.}
    if !self.auto_prestige_min_gain.is_positive(){self.auto_prestige_min_gain=Big::one();}
    if !self.auto_prestige_elapsed.is_finite()||self.auto_prestige_elapsed<0.{self.auto_prestige_elapsed=0.}
    if !self.auto_economy_elapsed.is_finite()||self.auto_economy_elapsed<0.{self.auto_economy_elapsed=0.}
    self.auto_economy_elapsed=self.auto_economy_elapsed.min(AUTO_PURCHASE_INTERVAL);
    for x in &mut self.total_mined{if *x<Big::zero(){*x=Big::zero()}}
    for x in &mut self.prestige{if *x<Big::zero(){*x=Big::zero()}}
    for x in &mut self.prestige_lifetime{if *x<Big::zero(){*x=Big::zero()}}
}
}
pub fn number(value:impl Into<Big>)->String{value.into().display()}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn buys_and_earns(){let mut g=Game::default();let old=g.cash.clone();assert_eq!(g.buy_miner(0,BuyAmount::One),1);g.tick(10.);assert!(g.cash>Big::zero());assert!(g.cash<old.add(&Big::from(100.)));}
    #[test]fn high_tier_beyond_f64(){let mut g=Game::default();g.prestige_lifetime[5]=Big::one();g.run_earned=n("1e500");assert!(g.unlocked(29));g.cash=n("8e800");g.miners[29]=1;g.equipment[29]=3;g.tick(1.);assert!(g.total_rate()>Big::zero());}
    #[test]fn cached_income_equals_individual_rates(){
        let mut g=Game::default();g.miners[0]=11;g.miners[14]=7;
        g.equipment[0]=3;g.research[0][0]=2;
        let expected=(0..COUNT).fold(Big::zero(),|acc,i|acc.add(&g.ore_rate(i)));
        assert_eq!(g.total_rate(),expected);
    }
    #[test]fn auto_gear_spends_and_upgrades(){
        let mut g=Game::default();g.cash=n("1e8");g.miners[0]=8;
        g.research[0][3]=1;g.auto_gear=true;
        g.tick(AUTO_PURCHASE_INTERVAL);
        g.auto_tick();
        assert!(g.equipment[0]>0);
    }
    #[test]fn automated_purchases_wait_at_least_one_minute(){
        let mut g=Game::default();g.cash=n("1e8");g.miners[0]=8;
        g.research[0][3]=1;g.auto_gear=true;
        g.auto_tick();
        g.tick(59.);
        g.auto_tick();
        assert_eq!(g.equipment[0],0);
        g.tick(1.);
        g.auto_tick();
        assert!(g.equipment[0]>0);
        let purchased=g.equipment[0];
        g.auto_tick();
        assert_eq!(g.equipment[0],purchased,"repeated polls must not buy again");
    }
    #[test]fn prestige_does_not_starve_automation_cadence(){
        let mut g=Game::default();
        g.auto_buy=true;g.research[0][2]=1;
        g.tick(30.);
        g.run_earned=n("1e6");
        assert!(g.ascend(0)>Big::zero());
        assert_eq!(g.auto_economy_elapsed,30.);
    }
    #[test]fn automatic_hire_and_gear_compete_on_real_marginal_roi(){
        let mut g=Game::default();
        g.cash=Big::from(1.);
        g.miners[0]=8;
        g.research[0][2]=1;
        g.research[0][3]=1;
        g.auto_buy=true;g.auto_gear=true;
        g.tick(AUTO_PURCHASE_INTERVAL);
        g.auto_tick();
        assert_eq!(g.equipment[0],1,"gear gives more income per dollar here");
        assert_eq!(g.miners[0],8,"the optimal upgrade leaves too little for hiring");
    }
    #[test]fn auto_purchase_waits_when_investment_cannot_repay(){
        let mut g=Game::default();
        g.cash=n("1e63");g.miners[0]=1;g.equipment[0]=100;
        g.research[0][3]=1;g.auto_gear=true;
        let previous_cash=g.cash.clone();
        g.tick(AUTO_PURCHASE_INTERVAL);
        g.auto_tick();
        // The upgrade would cost a large part of the treasury, but
        // contributes negligible revenue; save up for better options.
        assert_eq!(g.equipment[0],100);
        assert!(g.cash>=previous_cash);
    }
    #[test]fn research_upgrades(){let mut g=Game::default();g.prestige[0]=n("1e50");for k in 0..RESEARCH_COUNT{assert!(g.buy_research(0,k))}}
    #[test]fn prestige_reset(){let mut g=Game::default();g.run_earned=n("1e8");let gain=g.ascend(0);assert!(gain>=Big::one());assert_eq!(g.cash,Big::from(25.));}
    #[test]fn bulk_research_buying_handles_partial_quantities(){
        let mut g=Game::default();g.prestige[0]=n("1e100");
        assert_eq!(g.buy_research_bulk(0,0,100),100);
        assert_eq!(g.research[0][0],100);
        let mut g=Game::default();g.prestige[0]=g.research_bulk_cost(0,4,6);
        assert_eq!(g.buy_research_bulk(0,4,50),6);
        assert!(g.prestige[0].is_zero() || g.prestige[0]<g.research_cost(0,4));
    }
    #[test]fn utility_research_is_only_purchased_once(){
        let mut g=Game::default();g.prestige[0]=n("1e50");
        assert_eq!(g.buy_research_bulk(0,AUTO_RESEARCH_KIND,100),1);
        assert_eq!(g.research[0][AUTO_RESEARCH_KIND],1);
        assert_eq!(g.buy_research_bulk(0,AUTO_RESEARCH_KIND,100),0);
        assert_eq!(g.buy_research_bulk(0,7,100),9);
    }
    #[test]fn auto_research_requires_unlock_and_respects_funds(){
        let mut g=Game::default();g.prestige[0]=n("1e20");g.auto_research=true;
        assert_eq!(g.auto_research_tick(),0);
        g.research[0][AUTO_RESEARCH_KIND]=1;
        assert!(g.auto_research_tick()>0);
        assert!(g.research[0].iter().sum::<u32>()>1);
    }
    #[test]fn signature_upgrades_vary_by_prestige_tier(){
        assert_ne!(research_name(0,9),research_name(3,9));
        let mut g=Game::default();g.miners[0]=1;
        let base=g.total_rate();g.research[0][9]=3;
        assert!(g.total_rate()>base);
        g.research[0][10]=3;
        assert!(g.ore_rate(0)>base);
    }
    #[test]fn expands_to_sixty_ores_and_fourteen_prestiges(){
        assert_eq!(ORES.len(),60);assert_eq!(STAGES.len(),14);
        assert_eq!(ORES[29].name,"Void Crystal");assert_eq!(ORES[59].name,"Eternium");
        assert_eq!(STAGES[3].name,"Star Excavation");assert_eq!(STAGES[13].name,"Eternal Excavation");
        let mut g=Game::default();g.prestige_lifetime[13]=Big::one();g.run_earned=n("1e4600");assert!(g.unlocked(59));
    }
    #[test]fn automatic_prestige_needs_research_and_higher_current_tier(){
        let mut g=Game::default();g.auto_prestige=true;
        g.run_earned=n("1e80");g.auto_prestige_elapsed=50.;
        g.research[0][7]=1;
        assert!(g.auto_prestige_tick().is_none(),"current tier 0 cannot be auto-reset");
        g.prestige_lifetime[1]=Big::one();
        g.set_auto_prestige_min_gain("1e10").unwrap();
        assert_eq!(g.auto_prestige_tick().unwrap().0,0);
        assert_eq!(g.resets[0],1);
        assert!(g.auto_prestige_tick().is_none(),"cooldown must apply");
    }
    #[test]fn auto_prestige_selects_highest_eligible_below_current(){
        let mut g=Game::default();g.auto_prestige=true;g.research[4][7]=1;
        for i in 0..=4{g.prestige_lifetime[i]=n("1e8")}
        g.run_earned=n("1e300");g.auto_prestige_elapsed=60.;
        let (stage,_) = g.auto_prestige_tick().unwrap();
        assert_eq!(stage,3,"highest current tier must never auto-prestige");
        assert_eq!(g.resets[4],0);
    }
    #[test]fn auto_prestige_falls_back_to_other_lower_tiers(){
        let mut g=Game::default();g.auto_prestige=true;g.research[4][7]=1;
        for i in 0..=4{g.prestige_lifetime[i]=Big::from(100.)}
        g.run_earned=n("1e30");g.auto_prestige_elapsed=60.;
        let (stage,_) = g.auto_prestige_tick().unwrap();
        assert_eq!(stage,2,"tier 4 is current and tier 3 is too expensive");
    }
    #[test]fn minimum_gain_accepts_scientific_notation_and_rejects_invalid_input(){
        let mut g=Game::default();
        assert!(g.set_auto_prestige_min_gain("1e999999999999999999").is_ok());
        let old=g.auto_prestige_min_gain.clone();
        assert!(g.set_auto_prestige_min_gain("0").is_err());
        assert!(g.set_auto_prestige_min_gain("-2e3").is_err());
        assert!(g.set_auto_prestige_min_gain("nope").is_err());
        assert_eq!(old,g.auto_prestige_min_gain);
        g.set_auto_prestige_min_gain("25.5").unwrap();
        assert_eq!(g.auto_prestige_min_gain,Big::from(25.5));
    }
    #[test]fn after_tier_three_prestige_requirements_scale_exponentially(){
        // Tiers 1-3 deliberately retain the previous new-player economy.
        assert_eq!(STAGES[0].threshold,"1e6");
        assert_eq!(STAGES[1].threshold,"1e10");
        assert_eq!(STAGES[2].threshold,"1e23");
        let gaps:Vec<i64>=(3..PRESTIGE_COUNT).map(|i|{
            let a=STAGES[i].threshold[2..].parse::<i64>().unwrap();
            let b=STAGES[i-1].threshold[2..].parse::<i64>().unwrap();
            a-b
        }).collect();
        assert!(gaps[0]>=40);
        for pair in gaps.windows(2){
            assert!(pair[1]>=pair[0]*17/10,"exponent gaps must compound");
        }
        assert_eq!(Game::prestige_prerequisite(3),25);
        for tier in 4..PRESTIGE_COUNT {
            assert_eq!(Game::prestige_prerequisite(tier),Game::prestige_prerequisite(tier-1)*2);
        }
    }
    #[test]fn high_tiers_require_both_earnings_and_prior_prestige(){
        let mut game=Game::default();
        game.run_earned=n("1e100");
        assert!(!game.prestige_available(3));
        game.prestige_lifetime[2]=Big::from(25.);
        assert!(game.prestige_available(3));
        assert!(!game.prestige_available(4));
        game.prestige_lifetime[3]=Big::from(50.);
        game.run_earned=n("1e150");
        assert!(game.prestige_available(4));
    }
    #[test]fn high_prestige_balances_and_research_have_diminishing_returns(){
        let mut game=Game::default();
        game.prestige[2]=n("1e99");
        let softened=game.softened_prestige_balance(2);
        assert!(softened>n("1e30"));
        assert!(softened<n("1e40"));
        assert_eq!(effective_research_ranks(12),12);
        assert!(effective_research_ranks(100)<22);
        assert!(effective_research_ranks(u32::MAX)<50);
        assert_eq!(game.softened_prestige_balance(0),Big::zero());
    }
    #[test]fn late_prestige_gain_responds_less_to_overshooting(){
        let mut game=Game::default();
        game.prestige_lifetime[2]=Big::from(25.);
        game.run_earned=n("1e75");
        let reward=game.prestige_gain(3);
        assert!(reward>=Big::one());
        assert!(reward<n("1e4"));
    }
    #[test]fn high_prestige_preserves_high_currency_and_clears_lower(){
        let mut g=Game::default();g.prestige_lifetime[11]=n("1e6");
        g.prestige[2]=n("1e20");g.research[2][0]=1;
        g.prestige[13]=n("1e7");
        g.run_earned=n("1e65000");assert!(g.ascend(12)>=Big::one());
        assert!(g.prestige[2].is_zero());assert_eq!(g.research[2][0],0);
        assert_eq!(g.prestige[13],n("1e7"));
    }

    #[test]fn tier_one_remains_unchanged(){
        let mut g=Game::default();
        g.prestige_lifetime[0]=Big::one();
        g.run_earned=n("1e500");
        assert_eq!(g.current_prestige_tier(),0);
        assert!(g.unlocked(29),"Tier 1 ore access is unchanged");
        assert_eq!(g.progression_brake(n("1e165")),n("1e165"));
        assert_eq!(g.ore_growth(0),1.32);
    }
    #[test]fn tier_two_cannot_skip_to_extreme_ores(){
        let mut g=Game::default();
        g.prestige_lifetime[1]=Big::one();
        g.run_earned=n("1e200");
        g.cash=n("1e200");
        g.miners[25]=30;g.equipment[25]=80;
        assert_eq!(g.highest_accessible_ore(),10);
        assert!(!g.unlocked(25));
        assert!(g.ore_rate(25).is_zero(),"previously owned late ore stays dormant");
        assert_eq!(g.buy_miner(25,BuyAmount::Max),0);
        assert!(!g.buy_equipment(25));
        assert!(g.unlocked(10));
    }
    #[test]fn production_growth_is_compressed_but_unbounded(){
        let mut g=Game::default();
        g.prestige_lifetime[1]=Big::one();
        let a=g.progression_brake(n("1e50"));
        let b=g.progression_brake(n("1e165"));
        assert!(a>Big::zero());
        assert!(b>a,"softcap must not impose a fixed maximum");
        assert!(b<n("1e26"),"Tier 2 still compresses legacy 1e165/s production");
        g.prestige_lifetime[4]=Big::one();
        assert!(g.progression_brake(n("1e5000"))>Big::zero());
    }
    #[test]fn tier_two_excess_output_stays_well_below_next_threshold(){
        let mut g=Game::default();
        g.prestige_lifetime[1]=Big::one();
        // An unrealistic raw output from an old save is heavily compressed.
        let reduced=g.progression_brake(n("1e165"));
        assert!(reduced<n("1e26"));
        assert!(reduced>n("1e23"));
        // 1e23 is still achievable eventually: never impose a hard cap.
        assert!(g.progression_brake(n("1e100000"))>reduced);
    }
    #[test]fn progression_soft_knee_allows_realistic_earnings(){
        let mut g=Game::default();
        g.prestige_lifetime[1]=Big::one();
        // Tier 2 output of 1e20/s is no longer compressed by a knee at 1e16.
        assert_eq!(g.progression_brake(n("1e20")),n("1e20"));
        assert!(g.progression_brake(n("1e25"))>n("1e21"));
        g.prestige_lifetime[2]=Big::one();
        assert_eq!(g.progression_brake(n("1e60")),n("1e60"));
        assert!(g.progression_brake(n("1e100"))<n("1e100"));
    }
    #[test]fn ore_gates_advance_with_each_prestige(){
        let mut g=Game::default();
        g.prestige_lifetime[1]=Big::one();
        assert!(!g.ore_tier_available(29));
        g.prestige_lifetime[5]=Big::one();
        assert!(g.ore_tier_available(29));
        assert_eq!(Game::ore_required_tier(29),7-1);
        for tier in 1..PRESTIGE_COUNT {
            assert!(LAST_ORE_BY_TIER[tier]>=LAST_ORE_BY_TIER[tier-1].min(10));
        }
    }
}
