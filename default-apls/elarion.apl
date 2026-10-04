# Open with the equipped instant weapon ability and reuse it on cooldown.
actions=/weapon_shadow_mark
actions+=/weapon_frost_volley

# Assemble Elarion's damage window; spend affordable Spirit without overwriting Event Horizon.
actions+=/skystriders_grace
actions+=/event_horizon,if=!buff.event_horizon.up
actions+=/skystriders_supremacy,if=talent.bowguy-talent-id-talent13.enabled

# Without Fervent Supremacy, empty the short Multishot window immediately.
actions+=/multishot,if=!talent.bowguy-talent-id-talent13.enabled&buff.skystriders_supremacy.up

# Resurgent Winds makes Mark a charge generator; otherwise hold it for Barrage.
actions+=/lunarlight_mark,if=cooldown.heartseeker_barrage.ready|talent.bowguy-talent-id-talent3.enabled
actions+=/starfall_volley

# Reset Grace and Starfall early on one target, except in Final Crescendo builds.
actions+=/weapon_arcane_channel,if=targets.count=1&!talent.bowguy-talent-id-talent15.enabled&cooldown.skystriders_grace.remains>0&cooldown.starfall_volley.remains>0

# Consume the legendary ring proc before spending other rotational cooldowns.
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
