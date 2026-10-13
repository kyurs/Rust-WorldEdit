use crate::model::{self, Pos};
use pumpkin_plugin_api::{
    Context, Player, Plugin, PluginMetadata, Server, World,
    command::{Arg, Command, CommandError, CommandNode, CommandSender, ConsumedArgs},
    command_wit::{ArgumentType, StringType},
    commands::CommandHandler,
    common::{BlockPos, Hand},
    events::{
        EventData, EventHandler, EventPriority,
        block::{block_break::BlockBreakEvent, block_damage::BlockDamageEvent},
        player::player_interact::PlayerInteractEvent,
    },
    permission::{Permission, PermissionDefault, PermissionLevel},
    register_plugin,
    scheduler::SchedulerExt,
    text::TextComponent,
    wit::pumpkin::plugin::{event::InteractAction, item_stack::ItemStack},
    world,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
const HELP: &str = "Pumpkin WorldEdit Core (operators only)\n//wand: wooden axe; left click pos1, right click pos2\n//pos1 [X,Y,Z] | //pos2 [X,Y,Z] | //desel | //size\n//set <block> | //replace <from> <to>\n//copy | //paste [-a] | //undo | //redo\n//cancel | /wehelp\n4096 loaded blocks per edit; 8 undo levels. No containers, entities, brushes or schematics. History clears on restart.";
#[derive(Clone)]
struct Change {
    pos: Pos,
    before: u16,
    after: u16,
}
#[derive(Clone)]
struct History {
    dimension: String,
    cells: Vec<Change>,
}
#[derive(Clone)]
struct Clipboard {
    origin: Pos,
    cells: Vec<(Pos, u16)>,
}
#[derive(Clone, Default)]
struct Session {
    dimension: String,
    first: Option<Pos>,
    second: Option<Pos>,
    clipboard: Option<Clipboard>,
    undo: VecDeque<History>,
    redo: VecDeque<History>,
}
#[derive(Clone, Copy)]
enum Kind {
    Edit,
    Undo,
    Redo,
}
struct Job {
    owner: String,
    dimension: String,
    cells: Vec<Change>,
    cursor: usize,
    kind: Kind,
}
#[derive(Default)]
struct State {
    sessions: BTreeMap<String, Session>,
    job: Option<Job>,
    busy: bool,
    cancel: bool,
}
struct WorldEditCore {
    state: Arc<Mutex<State>>,
}
#[derive(Clone)]
struct Handler {
    state: Arc<Mutex<State>>,
    command: String,
}
fn key(p: &Player) -> String {
    let id = p.get_id();
    format!("{:016x}{:016x}", id.high, id.low)
}
fn op(p: &Player) -> bool {
    !matches!(p.get_permission_level(), PermissionLevel::Zero)
}
fn chat(p: &Player, message: &str) {
    p.send_system_message(TextComponent::text(message), false);
}
fn bp(p: Pos) -> BlockPos {
    BlockPos {
        x: p.0,
        y: p.1,
        z: p.2,
    }
}
fn xyz(p: BlockPos) -> Pos {
    (p.x, p.y, p.z)
}
fn player_pos(p: &Player) -> Pos {
    let (x, y, z) = p.get_position();
    (x.floor() as i32, y.floor() as i32, z.floor() as i32)
}
fn wand(p: &Player) -> bool {
    op(p)
        && p.get_item_in_hand(Hand::Right)
            .is_some_and(|s| model::is_wand_item(&s.get_registry_key()))
}
fn read(w: &World, p: Pos) -> Result<u16, String> {
    model::valid(p, &w.get_dimension())?;
    if w.get_chunk(p.0.div_euclid(16), p.2.div_euclid(16))
        .is_none()
    {
        return Err("All edited blocks must be in loaded chunks. Move closer first.".into());
    }
    let b = w.get_block_state(bp(p));
    // shortcut: reject block entities until a lossless NBT clipboard and undo implementation is added.
    if b.block_entity_type != u16::MAX || w.get_block_entity_nbt(bp(p)).is_some() {
        return Err(
            "This edit touches a block with stored data (container/sign/etc.); nothing queued."
                .into(),
        );
    }
    Ok(b.id)
}
fn material(raw: &str) -> Result<u16, String> {
    let (name, props) = model::material(raw)?;
    let id = world::resolve_block_state(&name, &props).ok_or("Unknown block or block state.")?;
    let info = world::block_state_to_info(id).ok_or("Unknown block state.")?;
    if !props.iter().all(|p| info.properties.contains(p)) {
        return Err("Unknown block property or value.".into());
    }
    if world::get_block_state_by_id(id)
        .ok_or("Unknown block state.")?
        .block_entity_type
        != u16::MAX
    {
        return Err("Blocks with stored data are not supported by this core port.".into());
    }
    Ok(id)
}
fn select(state: &Arc<Mutex<State>>, p: &Player, pos: Pos, first: bool) -> Result<String, String> {
    let dimension = p.get_world().get_dimension();
    model::valid(pos, &dimension)?;
    let mut s = state.lock().unwrap();
    if !s.sessions.contains_key(&key(p)) && s.sessions.len() >= 16 {
        return Err("WorldEdit session limit reached; restart clears inactive sessions.".into());
    }
    let session = s.sessions.entry(key(p)).or_default();
    if session.dimension != dimension {
        session.dimension = dimension;
        session.first = None;
        session.second = None;
    }
    let slot = if first {
        &mut session.first
    } else {
        &mut session.second
    };
    if *slot == Some(pos) {
        return Ok(String::new());
    }
    *slot = Some(pos);
    Ok(format!(
        "Position {} set to {},{},{}.",
        if first { 1 } else { 2 },
        pos.0,
        pos.1,
        pos.2
    ))
}
fn push(stack: &mut VecDeque<History>, item: History) {
    if !item.cells.is_empty() {
        stack.push_back(item);
        while stack.len() > 8 {
            stack.pop_front();
        }
    }
}
fn reverse(h: &History) -> Vec<Change> {
    h.cells
        .iter()
        .rev()
        .map(|c| Change {
            pos: c.pos,
            before: c.after,
            after: c.before,
        })
        .collect()
}
impl Handler {
    fn run(&self, p: &Player, raw: &str) -> Result<String, String> {
        if !op(p) {
            return Err("Only server operators can use WorldEdit.".into());
        }
        let w = p.get_world();
        let dimension = w.get_dimension();
        let id = key(p);
        let words: Vec<_> = raw.split_whitespace().collect();
        match self.command.as_str() {
            "help" => return Ok(HELP.into()),
            "pos1" | "pos2" => {
                return select(
                    &self.state,
                    p,
                    model::position(raw, player_pos(p), &dimension)?,
                    self.command == "pos1",
                );
            }
            "wand" => {
                if !words.is_empty() {
                    return Err("Usage: //wand".into());
                }
                // Main inventory slots 9..35 match the host's container slot numbering.
                let slot = (9..36)
                    .find(|&i| p.get_inventory_item(i).is_none_or(|s| s.get_count() == 0))
                    .ok_or("Make an empty slot in your main inventory first.")?;
                p.set_inventory_item(slot, Some(ItemStack::new("minecraft:wooden_axe", 1)));
                return Ok("Wooden axe added to your inventory. Hold it: left click selects pos1, right click selects pos2.".into());
            }
            "cancel" => {
                let mut s = self.state.lock().unwrap();
                if s.job.as_ref().is_some_and(|j| j.owner == id) {
                    s.cancel = true;
                    return Ok("Cancellation requested; completed blocks remain undoable.".into());
                }
                return Err("You have no active edit.".into());
            }
            _ => {}
        }
        let session = {
            let mut s = self.state.lock().unwrap();
            if s.busy {
                return Err(
                    "Another edit is running. Wait for completion or use //cancel for your edit."
                        .into(),
                );
            }
            if !s.sessions.contains_key(&id) && s.sessions.len() >= 16 {
                return Err("WorldEdit session limit reached.".into());
            }
            s.sessions.entry(id.clone()).or_default().clone()
        };
        if self.command == "desel" {
            let mut s = self.state.lock().unwrap();
            let v = s.sessions.get_mut(&id).unwrap();
            v.first = None;
            v.second = None;
            return Ok("Selection cleared.".into());
        }
        if matches!(self.command.as_str(), "undo" | "redo") {
            if !words.is_empty() {
                return Err("Usage: //undo or //redo (one operation at a time).".into());
            }
            let undo = self.command == "undo";
            let h = if undo {
                session.undo.back()
            } else {
                session.redo.back()
            }
            .ok_or("Nothing to undo/redo.")?;
            if h.dimension != dimension {
                return Err("Return to the edit's dimension first.".into());
            }
            let cells = reverse(h);
            for c in &cells {
                if read(&w, c.pos)? != c.before {
                    return Err("Undo/redo refused: blocks changed since that edit. Your history was preserved.".into());
                }
            }
            let mut s = self.state.lock().unwrap();
            let v = s.sessions.get_mut(&id).unwrap();
            if undo {
                v.undo.pop_back();
            } else {
                v.redo.pop_back();
            }
            let n = cells.len();
            s.job = Some(Job {
                owner: id,
                dimension,
                cells,
                cursor: 0,
                kind: if undo { Kind::Undo } else { Kind::Redo },
            });
            s.busy = true;
            s.cancel = false;
            return Ok(format!("Queued {n} blocks for {}.", self.command));
        }
        let mut changes = Vec::new();
        if self.command == "paste" {
            if !words.is_empty() && words != ["-a"] {
                return Err("Usage: //paste [-a] (skip air).".into());
            }
            let clip = session.clipboard.ok_or("Use //copy first.")?;
            let origin = player_pos(p);
            model::valid(origin, &dimension)?;
            for (pos, after) in clip.cells {
                if words == ["-a"] && world::get_block_state_by_id(after).is_some_and(|b| b.is_air)
                {
                    continue;
                }
                let target = (
                    origin.0 + pos.0 - clip.origin.0,
                    origin.1 + pos.1 - clip.origin.1,
                    origin.2 + pos.2 - clip.origin.2,
                );
                let before = read(&w, target)?;
                if before != after {
                    changes.push(Change {
                        pos: target,
                        before,
                        after,
                    });
                }
            }
        } else {
            if session.dimension != dimension {
                return Err("Select two positions in your current dimension first.".into());
            }
            let positions = model::region(
                session
                    .first
                    .ok_or("Set position 1 with //pos1 or the wand.")?,
                session
                    .second
                    .ok_or("Set position 2 with //pos2 or the wand.")?,
                &dimension,
            )?;
            match self.command.as_str() {
                "size" => return Ok(format!("Selection: {} blocks.", positions.len())),
                "copy" => {
                    if !words.is_empty() {
                        return Err("Usage: //copy".into());
                    }
                    model::valid(player_pos(p), &dimension)?;
                    let cells: Result<Vec<_>, String> = positions
                        .into_iter()
                        .map(|pos| Ok((pos, read(&w, pos)?)))
                        .collect();
                    let cells = cells?;
                    let n = cells.len();
                    self.state
                        .lock()
                        .unwrap()
                        .sessions
                        .get_mut(&id)
                        .unwrap()
                        .clipboard = Some(Clipboard {
                        origin: player_pos(p),
                        cells,
                    });
                    return Ok(format!(
                        "Copied {n} blocks relative to your position. Use //paste."
                    ));
                }
                "set" | "replace" => {
                    let replace = self.command == "replace";
                    if words.len() != if replace { 2 } else { 1 } {
                        return Err("Usage: //set <block> or //replace <from> <to>".into());
                    }
                    let after = material(words[words.len() - 1])?;
                    let from = if replace {
                        Some(material(words[0])?)
                    } else {
                        None
                    };
                    let from_block = from
                        .and_then(world::get_block_state_by_id)
                        .map(|b| b.block_id);
                    for pos in positions {
                        let before = read(&w, pos)?;
                        let matches = from.is_none()
                            || if words[0].contains('[') {
                                from == Some(before)
                            } else {
                                world::get_block_state_by_id(before).map(|b| b.block_id)
                                    == from_block
                            };
                        if matches && before != after {
                            changes.push(Change { pos, before, after });
                        }
                    }
                }
                _ => return Err(HELP.into()),
            }
        }
        if changes.is_empty() {
            return Ok("No blocks changed.".into());
        }
        let n = changes.len();
        let mut s = self.state.lock().unwrap();
        s.job = Some(Job {
            owner: id,
            dimension,
            cells: changes,
            cursor: 0,
            kind: Kind::Edit,
        });
        s.busy = true;
        s.cancel = false;
        Ok(format!("Queued {n} blocks. Use //undo after completion."))
    }
}
impl CommandHandler for Handler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        if let Some(p) = sender.as_player() {
            let raw = match args.get_value("settings") {
                Arg::Simple(s) => s,
                _ => String::new(),
            };
            match self.run(&p, &raw) {
                Ok(s) => {
                    if !s.is_empty() {
                        chat(&p, &s)
                    }
                }
                Err(e) => chat(&p, &e),
            };
        }
        Ok(1)
    }
}
fn tick(state: &Arc<Mutex<State>>, server: &Server) {
    let (job, cancel) = {
        let mut s = state.lock().unwrap();
        (s.job.take(), s.cancel)
    };
    let Some(mut job) = job else {
        return;
    };
    let player = server
        .get_all_players()
        .into_iter()
        .find(|p| key(p) == job.owner);
    let world = server
        .get_all_worlds()
        .into_iter()
        .find(|w| w.get_dimension() == job.dimension);
    let mut failure = if cancel {
        Some("Cancelled.")
    } else if player.as_ref().is_none_or(|p| !op(p)) {
        Some("Stopped: operator disconnected or lost operator status.")
    } else if world.is_none() {
        Some("Stopped: world unavailable.")
    } else {
        None
    };
    if failure.is_none() {
        let w = world.as_ref().unwrap();
        for _ in 0..64 {
            if job.cursor == job.cells.len() {
                break;
            }
            let c = &job.cells[job.cursor];
            if read(w, c.pos).ok() != Some(c.before) {
                failure = Some("Stopped: a block changed or its chunk unloaded.");
                break;
            }
            w.set_block_state(
                bp(c.pos),
                c.after,
                world::BlockFlags::NOTIFY_LISTENERS
                    | world::BlockFlags::FORCE_STATE
                    | world::BlockFlags::SKIP_DROPS,
            );
            if w.get_block_state(bp(c.pos)).id != c.after {
                failure = Some("Stopped: server could not apply a block change.");
                break;
            }
            job.cursor += 1;
        }
    }
    if failure.is_some() || job.cursor == job.cells.len() {
        let count = job.cursor;
        let remaining = job.cells.split_off(count);
        let mut s = state.lock().unwrap();
        let v = s.sessions.get_mut(&job.owner).unwrap();
        if !remaining.is_empty() && !matches!(job.kind, Kind::Edit) {
            let h = History {
                dimension: job.dimension.clone(),
                cells: remaining,
            };
            let restored = History {
                dimension: job.dimension.clone(),
                cells: reverse(&h),
            };
            push(
                if matches!(job.kind, Kind::Undo) {
                    &mut v.undo
                } else {
                    &mut v.redo
                },
                restored,
            );
        }
        let applied = History {
            dimension: job.dimension,
            cells: job.cells,
        };
        match job.kind {
            Kind::Edit => {
                if count > 0 {
                    v.redo.clear();
                }
                push(&mut v.undo, applied)
            }
            Kind::Undo => push(&mut v.redo, applied),
            Kind::Redo => push(&mut v.undo, applied),
        }
        s.busy = false;
        s.cancel = false;
        drop(s);
        if let Some(p) = player {
            chat(
                &p,
                &format!(
                    "{} {count} blocks changed.{}",
                    failure.unwrap_or("Edit complete."),
                    if count > 0 { " History saved." } else { "" }
                ),
            );
        }
    } else {
        state.lock().unwrap().job = Some(job);
    }
}
impl EventHandler<PlayerInteractEvent> for Handler {
    fn handle(
        &self,
        _server: Server,
        mut event: EventData<PlayerInteractEvent>,
    ) -> EventData<PlayerInteractEvent> {
        if !event.cancelled
            && wand(&event.player)
            && let Some(pos) = event.clicked_pos
            && matches!(
                event.action,
                InteractAction::LeftClickBlock | InteractAction::RightClickBlock
            )
        {
            event.cancelled = true;
            let result = select(
                &self.state,
                &event.player,
                xyz(pos),
                matches!(event.action, InteractAction::LeftClickBlock),
            );
            let message = result.unwrap_or_else(|s| s);
            if !message.is_empty() {
                chat(&event.player, &message);
            }
        }
        event
    }
}
impl EventHandler<BlockDamageEvent> for Handler {
    fn handle(
        &self,
        _server: Server,
        mut event: EventData<BlockDamageEvent>,
    ) -> EventData<BlockDamageEvent> {
        if !event.cancelled && wand(&event.player) {
            event.cancelled = true;
            let message = select(&self.state, &event.player, xyz(event.block_pos), true)
                .unwrap_or_else(|s| s);
            if !message.is_empty() {
                chat(&event.player, &message);
            }
        }
        event
    }
}
impl EventHandler<BlockBreakEvent> for Handler {
    fn handle(
        &self,
        _server: Server,
        mut event: EventData<BlockBreakEvent>,
    ) -> EventData<BlockBreakEvent> {
        if event.player.as_ref().is_some_and(wand) {
            event.cancelled = true;
        }
        event
    }
}
impl Plugin for WorldEditCore {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
        }
    }
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "WorldEditCore".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["KYURS".into()],
            description: "Operator-only native WorldEdit core commands for Pumpkin".into(),
            dependencies: vec![],
            permissions: vec![],
        }
    }
    fn on_load(&self, context: Context) -> pumpkin_plugin_api::Result<()> {
        context.register_permission(&Permission {
            node: "WorldEditCore:edit".into(),
            description: "World editing (also requires actual operator status)".into(),
            default: PermissionDefault::Op(PermissionLevel::One),
            children: vec![],
        })?;
        for name in [
            "wand", "pos1", "pos2", "desel", "size", "set", "replace", "copy", "paste", "undo",
            "redo", "cancel", "help",
        ] {
            let h = Handler {
                state: self.state.clone(),
                command: name.into(),
            };
            context.register_command(
                // The pinned host strips a slash from modern command packets; retain both spellings.
                Command::new(
                    &[
                        format!("/{name}"),
                        if name == "help" {
                            "wehelp".into()
                        } else {
                            name.into()
                        },
                    ],
                    "Operator-only WorldEdit core",
                )
                .execute(h.clone())
                .then(
                    CommandNode::argument("settings", &ArgumentType::String(StringType::Greedy))
                        .execute(h),
                ),
                "WorldEditCore:edit",
            );
        }
        let h = Handler {
            state: self.state.clone(),
            command: String::new(),
        };
        context.register_event_handler::<PlayerInteractEvent, _>(
            h.clone(),
            EventPriority::Highest,
            true,
        )?;
        context.register_event_handler::<BlockDamageEvent, _>(
            h.clone(),
            EventPriority::Highest,
            true,
        )?;
        context.register_event_handler::<BlockBreakEvent, _>(h, EventPriority::Highest, true)?;
        let state = self.state.clone();
        context.schedule_repeating_task(1, 1, move |server| tick(&state, &server));
        Ok(())
    }
}
register_plugin!(WorldEditCore);
