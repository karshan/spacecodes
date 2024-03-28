#Features
 * server sets constants (GAME_MAP, INTERCEPT_COST, PASSIVE_GOLD_GAIN etc.)
 * bounty runes 100/200g
 * Visual feedback on spell icons when pressing hotkey
 * Allow clicking spell icons
 * draw intercept circle when targeting?
 * Make intercept upgrades actually do something
 * Message death animation
 * Show text error message if trying to intercept without enough gold/too close to enemy unit

#Balance
 * Reduce passive gold income

#Ideas
 * you only get gold bounties if the message successfully delivers
 * blink bounties
 * crossing rivers increases fuel gain for message
 * protection bubble is bigger but only in direction unit is moving
 * can intercept while drawing path
 * can stash incomplete/complete paths for later
 * turn based path drawing. 30 second path draw phase followed by interception phase ?
 * intercept circle grows with time
 * bubble radius increase upgrade
 * can only intercept after unit turns

#Bugs
 * serialize_state() mismatch after ingame restart
 * message and blinking message should be the same shape
 * Recover when packets from both clients are dropped on the same frame
 * [P4] move_unit() moves slower around turns
 * It is possible to send 2+ blink commands or intercept commands etc. in the same frame because the cooldown/gold doesn't update until apply_updates() is called

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