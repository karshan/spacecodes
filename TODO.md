#Balance
 * [P1] Passive lumber 1/5sec when below 5
 * [P1] Balance gold and fuel
    * 10 kills to win. or 10 kills gets you some upgrade.
    * More fuel loss
    * Less fuel gain from fuel bounty
 * More bounties in general. and more bounties as time passes.

#Visuals
* [P1] Show {message spawn, intercept, blink} spell and cooldown
* [P1] Show lumber cost
* [P1] visual fuel indication
* [P1] overlapping messages are visually glitchy
* [P3] Message death animation (stop and fade out)
* [P3] Better visuals for showing bounties/buffs carried by messages
* [P3] Intercept mine animation
* [P3] Make paths shiny while they are being drawn (move light?)


#Features
 * [P1] Handle low frame_rate
 * [P1] Unit selection rect only needs to intersect with message
 * Path won't complete if a segment goes through station but end point/turnpoint are not in station
  
 * [P2] Show text error message if trying to intercept without enough gold/too close to enemy unit
 * [P2] Give an indication when not enough lumber for path

 * [P3] Easily playable online
    * TCP + UDP. UDP only for game input
    * Binary should check version with server on launch
    * Enter lobby code to join game
    * Ready -> 3,2,1
    * Host binary somewhere
 * [P3] latency based frame_delay
    * send packets only every other frame
    * Latency measurement doesnt account for out of order packets
 * [P3] Recover when packets from both clients are dropped on the same frame
 * [P3] "Cast Animations" for blink, message spawn, intercept. To ease latency.

 * [P3] Grapple (short range power shot to grab buffs)
 * [P3] Sound

#Ideas
 * crossing rivers increases fuel gain for message/speed etc.
 * can intercept while drawing path
 * can stash incomplete/complete paths for later
 * Consider adding "undo last path point"

#Hygiene
 * restructure main()
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
 * render shadows in a separate pass
 * profiling (flamegraph + dtrace)
 * No/minumum heap use
 * Packed bools and structs
 * maximise cache utilization, iterate through units just once per frame
 * function inlining?