# Cash out stored Rend and Blood Feathers before the encounter ends.
actions=/owed_in_blood,if=resource.blood_feathers.current>0&fight.remains<5
actions+=/slaughter,if=debuff.rend.up&fight.remains<4

# Prepare the burst window: Rupture leads in cleave, while Blood Arc leads at low target counts.
actions+=/rupture,if=(targets.count>=3|talent.gunde-talent-id-talent13.enabled)&resource.spirit.current=resource.spirit.max&cooldown.reign_in_blood.ready&cooldown.blood_arc.ready
actions+=/blood_arc,if=resource.spirit.current=resource.spirit.max&cooldown.reign_in_blood.ready&(targets.count<3|buff.open_wounds.up)
actions+=/bloodbound_spirit,if=resource.spirit.current=resource.spirit.max&cooldown.reign_in_blood.ready&buff.serrated_edge.up&((legendary.wrists_a_criticalstrike.equipped&talent.gunde-talent-id-talent13.enabled&buff.deaths_arc.up&buff.harvesters_toll.up)|!legendary.wrists_a_criticalstrike.equipped|!talent.gunde-talent-id-talent13.enabled)

# Bloodcraze spends feathers before Reign in Blood. Carrion Onslaught waits for Death's Arc inside Harvester's Toll.
actions+=/owed_in_blood,if=talent.gunde-talent-id-talent13.enabled&buff.bloodbound_spirit.up&cooldown.rupture.remains>0&resource.blood_feathers.current>0&(!legendary.wrists_a_criticalstrike.equipped|(buff.deaths_arc.up&buff.harvesters_toll.up))
actions+=/reign_in_blood,if=buff.bloodbound_spirit.up
actions+=/blood_arc,if=legendary.wrists_a_criticalstrike.equipped&buff.carrion_onslaught.up

# Put the equipped weapon inside Reign in Blood when the full burst window is active.
actions+=/weapon_shadow_mark,if=buff.reign_in_blood.up
actions+=/weapon_cleave_charge,if=buff.reign_in_blood.up
actions+=/weapon_instant_aoe,if=buff.reign_in_blood.up
actions+=/weapon_frontal_cone,if=buff.reign_in_blood.up

actions+=/owed_in_blood,if=!talent.gunde-talent-id-talent13.enabled&buff.reign_in_blood.up&cooldown.rupture.remains>0&resource.blood_feathers.current>0&((targets.count<3&cooldown.heart_splitter.ready)|(targets.count>=3&cooldown.grim_carve.remains>0&cooldown.heart_splitter.ready))
actions+=/owed_in_blood,if=resource.blood_feathers.current=resource.blood_feathers.max&cooldown.rupture.remains>0

# Feed Serrated Edge into Heart Splitter below three targets and Grim Carve in cleave.
actions+=/rupture,if=targets.count>=4&cooldown.slaughter.remains<18
actions+=/blood_arc,if=targets.count>=3&cooldown.grim_carve.remains<2
actions+=/blood_arc,if=targets.count<3&(cooldown.rupture.remains<2|cooldown.heart_splitter.remains<2)
actions+=/rupture,if=targets.count<3&cooldown.slaughter.remains<18
# Bleeding Hearts cashes in its guaranteed high-health critical strikes; Bloody Mess promotes Grim Carve in its focused build.
actions+=/heart_splitter,if=legendary.ring_c_criticakstrike_expertise.equipped&target.health.pct>80
actions+=/grim_carve,if=legendary.back_a_haste.equipped&targets.count<3
actions+=/heart_splitter,if=targets.count<3
actions+=/grim_carve

# Outside the full opener, use the weapon after a priority rotational ability.
actions+=/weapon_shadow_mark
actions+=/weapon_cleave_charge
actions+=/weapon_instant_aoe
actions+=/weapon_frontal_cone

actions+=/rupture,if=targets.count=3&cooldown.slaughter.remains<18
actions+=/reavers_edge,if=targets.count>=4
actions+=/heart_splitter
actions+=/reavers_edge

# Consume Rend only after the primary rotational cooldowns have been used.
actions+=/slaughter,if=debuff.rend.up&!buff.reign_in_blood.up&cooldown.heart_splitter.remains>0&cooldown.grim_carve.remains>0&cooldown.blood_arc.remains>0&(buff.open_wounds.up|targets.count>=3|cooldown.rupture.remains>10)
actions+=/double_strike
