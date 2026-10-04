# Establish Seething Poison with a stealthed Widow's Bite.
actions=/brooding_shadows,if=!buff.brooding_shadows.up&debuff.seething_poison.remains<6
actions+=/widows_bite,if=buff.brooding_shadows.up&debuff.seething_poison.remains<6

# Establish the bleed before the main single-target damage window, or in any From the Shadows build.
actions+=/hemorrhaging_strike,if=(targets.count<3|legendary.back_a_haste.equipped)&!dot.hemorrhaging_strike.up&resource.combo_points.current>=4

# Use the equipped weapon before Matriarch and Maiden when their windows align.
actions+=/weapon_shadow_mark
actions+=/weapon_cleave_charge
actions+=/weapon_instant_aoe
actions+=/weapon_frontal_cone

# Spend the equipped Spirit cost without replacing an active Matriarch.
actions+=/matriarch_macabre,if=!buff.matriarch_macabre.up
actions+=/maiden_of_death
# In cleave, cast Maiden first so its bleed application benefits Hemorrhaging Strike.
actions+=/hemorrhaging_strike,if=targets.count>=3&!dot.hemorrhaging_strike.up&resource.combo_points.current>=4
actions+=/final_stratagem,if=talent.mara-talent-id-talent8.enabled&buff.matriarch_macabre.up&buff.maiden_of_death.up
actions+=/final_stratagem,if=!talent.mara-talent-id-talent8.enabled&!buff.maiden_of_death.up&cooldown.brooding_shadows.remains>0

# Consume Hemotoxin promptly and otherwise maintain the bleed at the right spender threshold.
actions+=/hemorrhaging_strike,if=targets.count>=3&debuff.hemotoxin.up&resource.combo_points.current>=4
actions+=/hemorrhaging_strike,if=targets.count<3&debuff.hemotoxin.up&resource.combo_points.current>=5
actions+=/hemorrhaging_strike,if=targets.count>=3&dot.hemorrhaging_strike.remains<3&resource.combo_points.current>=4
actions+=/hemorrhaging_strike,if=targets.count<3&dot.hemorrhaging_strike.remains<3&resource.combo_points.current>=5

# Use the appropriate stealth poison, then spend with the target-count finisher.
actions+=/brooding_shadows,if=!buff.brooding_shadows.up&targets.count>=4&resource.energy.current>=35
actions+=/skittering_blades,if=targets.count>=4&buff.brooding_shadows.up
actions+=/brooding_shadows,if=!buff.brooding_shadows.up&targets.count<4&resource.combo_points.current=0&resource.energy.current>=20
actions+=/backstab,if=targets.count<4&buff.brooding_shadows.up
# Consume Malevolence with the opposite finisher before the target-count default.
actions+=/arachnid_assault,if=targets.count<3&buff.malevolence_arachnid.up&resource.combo_points.current>=5
actions+=/queens_fang,if=targets.count>=3&buff.malevolence_queen.up&resource.combo_points.current>=4
actions+=/arachnid_assault,if=targets.count>=3&resource.combo_points.current>=4
actions+=/queens_fang,if=targets.count<3&resource.combo_points.current>=5

# Avoid capping Widow's Bite charges, then use the correct builder for the pack size.
actions+=/widows_bite,if=cooldown.widows_bite.charges=3&resource.combo_points.deficit>=2
actions+=/skittering_blades,if=targets.count>=3&resource.combo_points.current<4&resource.energy.current>=35
actions+=/backstab,if=targets.count<3&resource.combo_points.current<5
actions+=/mara_attack
