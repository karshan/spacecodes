#Features
 * [P2] server sets constants (GAME_MAP, INTERCEPT_COST, PASSIVE_GOLD_GAIN etc.)
 * [P3] Show text error message if trying to intercept without enough gold/too close to enemy unit
 * [P3] draw intercept circle when targeting?
 * [P3] Message death animation
 * [P4] Visual feedback on spell icons when pressing hotkey
 * [P4] Allow clicking spell icons

#Ideas
 * crossing rivers increases fuel gain for message/speed etc.
 * can intercept while drawing path
 * can stash incomplete/complete paths for later

#Bugs
 * [P3] Recover when packets from both clients are dropped on the same frame
 * [P4] message and blinking message should be the same shape (collision still assumes rectangle)
 * [P4] move_unit() moves slower around turns

#Hygiene
 * game_state.{my_units, other_units} -> game_state.units: [Vec<Unit>; 2]
 * Fix main::serialize_state() for game_state.upgrades and .items
 * Unit.cooldown should only exist for blinking messages
 * ShipSpellIcons should be HashMap<ShipSpells, Icon>
 * shop ui code isn't great it should probably use a HashMap<ShopItem, Icon>
 * game_state.selection type should be isomorhpic to (bool, bool, Vec<unit_id>)
 * unit.blinking: Option<bool> is ugly make a better type for this
 * Use with_serde feature of raylib to get Vector2 serde instance
 * Remove unit.dead, use a to_kill: Vec<unit_id> whenever units can die and update them immediately

#Performance
 * No/minumum heap use
 * Packed bools and structs
 * maximise cache utilization, iterate through units just once per frame
 * function inlining?
 * frame type i64->u32