#Balance
 * [P1] Passive lumber 1/5sec when below 5
 * [P1] Balance gold and fuel
    * 10 kills to win. or 10 kills gets you some upgrade.
    * More fuel loss
    * Less fuel gain from fuel bounty

#Features
 * [P1] latency based frame_delay
    * send packets only every other frame
    * Latency measurement doesnt account for out of order packets
 * [P1] Easily playable online
    * Binary should check version with server on launch
    * Enter lobby code to join game
    * Ready -> 3,2,1
    * Host binary somewhere
 * [P1] Fullscreen/resolution scaling
 * [P1] Handle low frame_rate
 * [P1] Path's drawn as rectangle not thick lines
 * [P1] Map visual overhaul
 * [P2] Recover when packets from both clients are dropped on the same frame
 * [P2] Message death animation (stop and fade out)
 * [P2] Show text error message if trying to intercept without enough gold/too close to enemy unit
 * [P2] Give an indication when not enough lumber for path
 * [P3] "Cast Animations" for blink, message spawn, intercept. To ease latency.
 * [P3] Grapple (short range power shot to grab buffs)
 * [P3] Sound
 * [P3] Better intercept cursor ?
 * [P4] Ensure bounties don't spawn one sided (location)
 * [P4] Experiment flags for new features like grapple.
 * [P4] Allow clicking spell icons

#Ideas
 * crossing rivers increases fuel gain for message/speed etc.
 * can intercept while drawing path
 * can stash incomplete/complete paths for later
 * Consider adding "undo last path point"

#Bugs
 * [P3] Recover when packets from both clients are dropped on the same frame
 * [P4] move_unit() moves slower around turns

#Hygiene
 * Add probability sampling util functions and use them in add_bounty
 * Use bounty_enum::len() in add_bounty()
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
 * profiling
 * No/minumum heap use
 * Packed bools and structs
 * maximise cache utilization, iterate through units just once per frame
 * function inlining?
 * frame type i64->u32