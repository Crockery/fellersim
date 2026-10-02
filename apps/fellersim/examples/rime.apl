# Open with the equipped instant weapon ability and reuse it on cooldown.
actions=/weapon_shadow_mark
actions+=/weapon_frost_volley

# Use major damage buffs as soon as they are available.
actions+=/wrath_of_winter,if=resource.spirit.current=resource.spirit.max
actions+=/ice_blitz
actions+=/winters_blessing
actions+=/flight_of_the_navir

# Avoid capping Cold Snap before preparing the next Bursting Ice window.
actions+=/cold_snap,if=cooldown.cold_snap.charges=2&resource.winter_orbs.current<resource.winter_orbs.max&(targets.count>=3|!cooldown.bursting_ice.ready)
actions+=/freezing_torrent,if=(targets.count>=3|(targets.count>=2&talent.rime-talent-id-talent6.enabled))&cooldown.bursting_ice.ready&resource.winter_orbs.current<4
actions+=/bursting_ice

# After the opener spends Bursting Ice, recover the committed cooldowns with Chronoshift.
actions+=/weapon_arcane_channel,if=cooldown.bursting_ice.remains>0&cooldown.ice_blitz.remains>0&cooldown.winters_blessing.remains>0&cooldown.flight_of_the_navir.remains>0

# Nature's Fury is pure damage in this scenario, so use it before the orb spenders.
actions+=/weapon_chain_lightning

# Consume a fully stacked Glacial Assault proc before spending Winter Orbs.
actions+=/glacial_blast,if=talent.rime-talent-id-talent1.enabled&buff.glacial_assault.stacks>=4

# Icy Talons spends at four Orbs, or five with Undulating Spirit; Skandi extends its Bursting Ice window.
actions+=/ice_comet,if=talent.rime-talent-id-talent17.enabled&(targets.count>=3|(targets.count>=2&talent.rime-talent-id-talent6.enabled))&((legendary.ring_c_criticakstrike_expertise.equipped&resource.winter_orbs.current=resource.winter_orbs.max)|(!legendary.ring_c_criticakstrike_expertise.equipped&resource.winter_orbs.current>=4))&(cooldown.bursting_ice.remains>7|(legendary.wrists_a_haste.equipped&cooldown.bursting_ice.remains>5)|resource.winter_orbs.current=resource.winter_orbs.max|buff.wrath_of_winter.up|fight.remains<5)
actions+=/glacial_blast,if=talent.rime-talent-id-talent17.enabled&(targets.count<2|(targets.count<3&!talent.rime-talent-id-talent6.enabled))&((legendary.ring_c_criticakstrike_expertise.equipped&resource.winter_orbs.current=resource.winter_orbs.max)|(!legendary.ring_c_criticakstrike_expertise.equipped&resource.winter_orbs.current>=4))&(cooldown.bursting_ice.remains>7|(legendary.wrists_a_haste.equipped&cooldown.bursting_ice.remains>5)|resource.winter_orbs.current=resource.winter_orbs.max|buff.wrath_of_winter.up|fight.remains<5)
actions+=/ice_comet,if=!talent.rime-talent-id-talent17.enabled&(targets.count>=3|(targets.count>=2&talent.rime-talent-id-talent6.enabled))&resource.winter_orbs.current>=2&(cooldown.bursting_ice.remains>7|resource.winter_orbs.current=resource.winter_orbs.max|buff.wrath_of_winter.up|buff.frostweavers_wrath.up|buff.icy_flow.up|fight.remains<5)
actions+=/glacial_blast,if=!talent.rime-talent-id-talent17.enabled&(targets.count<2|(targets.count<3&!talent.rime-talent-id-talent6.enabled))&resource.winter_orbs.current>=2&(cooldown.bursting_ice.remains>7|resource.winter_orbs.current=resource.winter_orbs.max|buff.wrath_of_winter.up|buff.frostweavers_wrath.up|buff.icy_flow.up|fight.remains<5)

# Rebuild Orbs with Torrent in cleave, or when the single-target spender is unavailable.
actions+=/freezing_torrent,if=(targets.count<2|(targets.count<3&!talent.rime-talent-id-talent6.enabled))&resource.winter_orbs.current<2
actions+=/freezing_torrent,if=(targets.count>=3|(targets.count>=2&talent.rime-talent-id-talent6.enabled))&resource.winter_orbs.current<4
actions+=/cold_snap,if=resource.winter_orbs.current<resource.winter_orbs.max
actions+=/frost_bolt
