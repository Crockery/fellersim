# Apply Voidbringer immediately and prime buff-granting weapons for the next lightning window.
actions=/weapon_shadow_mark
actions+=/weapon_cleave_charge,if=cooldown.thunder_call.ready|buff.thunder_call.up|buff.raging_tempest.up
actions+=/weapon_instant_aoe,if=cooldown.thunder_call.ready|buff.thunder_call.up|buff.raging_tempest.up

# Alternate Thunder Call and Raging Tempest for sustained lightning windows.
actions+=/raging_tempest,if=resource.spirit.current=resource.spirit.max&!buff.thunder_call.up
actions+=/thunder_call,if=!buff.raging_tempest.up

# Earthbreaker contributes direct area damage after the lightning window is active.
actions+=/weapon_frontal_cone

# Focused Wrath is immediate in Thunder Call; during Raging Tempest, let expertise build first.
actions+=/focused_wrath,if=buff.thunder_call.up&!buff.raging_tempest.up
actions+=/focused_wrath,if=buff.raging_tempest.up&buff.raging_tempest.remains<10

# Healthy training dummies allow Culling Strike only through Executioner's Grin.
actions+=/culling_strike,if=legendary.wrists_a_expertise.equipped&buff.executioners_grin.up

# Slayer's Mosh makes Leap a burst-window opener; otherwise it starts High Road's builder cycle or grants Focused Wrath through Mouth for War.
actions+=/leap_smash,if=legendary.back_a_haste.equipped|(talent.ink-talent-id-talent14.enabled&resource.fury.deficit>=20)|(talent.ink-talent-id-talent5.enabled&resource.fury.current>=25)

# Kill 'Em All makes the next two Heavy Strikes the highest-priority builders.
actions+=/heavy_strike,if=buff.kill_em_all.up

# Schism always favors Skull Crusher unless Hammer Storm has an empowered proc to consume.
actions+=/hammer_storm,if=talent.ink-talent-id-talent9.enabled&buff.schism_hammer_storm.up&(targets.count>=3|buff.focused_wrath.up|buff.schism_hammer_storm.stacks=2|buff.schism_hammer_storm.remains<3)&(buff.thunder_call.up|resource.fury.current=resource.fury.max|fight.remains<5)
actions+=/skull_crusher,if=talent.ink-talent-id-talent9.enabled&(buff.thunder_call.up|resource.fury.current=resource.fury.max|fight.remains<5)

# Lightning builds use Hammer Storm in cleave and Skull Crusher below three targets.
actions+=/hammer_storm,if=!talent.ink-talent-id-talent9.enabled&targets.count>=3&(buff.thunder_call.up|resource.fury.current=resource.fury.max|fight.remains<5)
actions+=/skull_crusher,if=!talent.ink-talent-id-talent9.enabled&targets.count<3&(buff.thunder_call.up|resource.fury.current=resource.fury.max|fight.remains<5)

# Lightning prioritizes Chain Lightning; Schism moves it behind the melee builders in cleave.
actions+=/tariq_chain_lightning,if=!talent.ink-talent-id-talent9.enabled|targets.count<3
actions+=/heavy_strike,if=(!talent.ink-talent-id-talent9.enabled|targets.count<3)&resource.fury.deficit>=12
actions+=/face_breaker,if=(!talent.ink-talent-id-talent9.enabled|targets.count<3)&resource.fury.deficit>=7
actions+=/face_breaker,if=talent.ink-talent-id-talent9.enabled&targets.count>=3&resource.fury.deficit>=7
actions+=/heavy_strike,if=talent.ink-talent-id-talent9.enabled&targets.count>=3&resource.fury.deficit>=12
actions+=/tariq_chain_lightning,if=talent.ink-talent-id-talent9.enabled&targets.count>=3
actions+=/wild_swing
