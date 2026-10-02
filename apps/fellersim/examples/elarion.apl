# Open with the equipped instant weapon ability and reuse it on cooldown.
actions=/weapon_shadow_mark
actions+=/weapon_frost_volley

# Assemble Elarion's major damage window as its pieces become available.
actions+=/skystriders_grace
actions+=/event_horizon,if=resource.spirit.current=resource.spirit.max
actions+=/skystriders_supremacy,if=talent.bowguy-talent-id-talent13.enabled

# Without Fervent Supremacy, empty the short Multishot window immediately.
actions+=/multishot,if=!talent.bowguy-talent-id-talent13.enabled&buff.skystriders_supremacy.up

# Hold Lunarlight Mark for Barrage, with Starfall active before the channel.
actions+=/lunarlight_mark,if=cooldown.heartseeker_barrage.ready
actions+=/starfall_volley
actions+=/heartseeker_barrage,if=legendary.ring_c_spirit_spirit.equipped&buff.impending_heartseeker.up

# Consume Resurgent Winds, establish Shimmer, and avoid capping Highwind Arrow charges.
actions+=/highwind_arrow,if=buff.resurgent_winds.up
actions+=/highwind_arrow,if=!talent.bowguy-talent-id-talent14.enabled&cooldown.highwind_arrow.charges=3
actions+=/highwind_arrow,if=legendary.back_a_haste.equipped&cooldown.highwind_arrow.charges>=2

# Impending Heartseeker identifies the Barrage build; otherwise favor Highwind Arrow.
actions+=/multishot,if=!talent.bowguy-talent-id-talent14.enabled&targets.count>=3&(buff.empowered_multishot.up|buff.skystriders_supremacy.up)
actions+=/heartseeker_barrage,if=talent.bowguy-talent-id-talent14.enabled
actions+=/heartseeker_barrage,if=!talent.bowguy-talent-id-talent14.enabled&targets.count<3
actions+=/multishot,if=legendary.wrists_a_criticalstrike.equipped&dot.starfall_volley.up
actions+=/highwind_arrow,if=!talent.bowguy-talent-id-talent14.enabled&targets.count>=3

# Channel Chronoshift only after the primary Grace and Starfall cooldowns are committed.
actions+=/weapon_arcane_channel,if=cooldown.skystriders_grace.remains>0&cooldown.starfall_volley.remains>0

# Nature's Fury is pure damage in this scenario, so use it ahead of rotational filler.
actions+=/weapon_chain_lightning

# Spend procs, then activate the shorter base Supremacy window at its priority point.
actions+=/celestial_shot,if=buff.celestial_impetus.up
actions+=/skystriders_supremacy,if=!talent.bowguy-talent-id-talent13.enabled
actions+=/multishot,if=buff.empowered_multishot.up|buff.skystriders_supremacy.up

# Finish each build's target-count priority while preserving enough Focus for cooldowns.
actions+=/multishot,if=targets.count>=3&resource.focus.current>=50
actions+=/highwind_arrow,if=talent.bowguy-talent-id-talent14.enabled
actions+=/heartseeker_barrage,if=!talent.bowguy-talent-id-talent14.enabled&targets.count>=3
actions+=/highwind_arrow,if=!talent.bowguy-talent-id-talent14.enabled
actions+=/focused_shot,if=resource.focus.current<50
actions+=/celestial_shot,if=resource.focus.current>85
actions+=/focused_shot
