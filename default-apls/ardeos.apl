# Open with the equipped instant weapon ability and reuse it on cooldown.
actions=/weapon_shadow_mark
actions+=/weapon_frost_volley

# Spend Embers during Wildfire, or before the fight ends.
actions+=/detonate,if=buff.wildfire.up|fight.remains<5

# Assemble the full burn window with four stored Embers.
actions+=/fire_frogs,if=cooldown.wildfire.ready&cooldown.apocalypse.ready&cooldown.fire_ball.charges>=1&cooldown.engulfing_flames.charges=2&(resource.embers.current=resource.embers.max|fight.remains<20)
actions+=/apocalypse,if=cooldown.wildfire.ready&cooldown.fire_frogs.remains>0
actions+=/fire_ball,if=cooldown.wildfire.ready&cooldown.apocalypse.remains>0&cooldown.fire_ball.charges>=1&dot.fire_ball.remains<6
actions+=/engulfing_flames,if=cooldown.wildfire.ready&cooldown.apocalypse.remains>0&cooldown.fire_ball.charges<2
actions+=/pyromania,if=cooldown.wildfire.ready&cooldown.engulfing_flames.charges=0
actions+=/incinerate,if=cooldown.wildfire.ready&cooldown.engulfing_flames.charges=0&resource.spirit.current=resource.spirit.max&!dot.incinerate.up
actions+=/wildfire,if=cooldown.apocalypse.remains>0&cooldown.engulfing_flames.charges=0

# After the Wildfire burn, spend Fire Ball charges that Chronoshift will recover, then channel it.
actions+=/fire_ball,if=cooldown.weapon_arcane_channel.ready&cooldown.wildfire.remains>0&!buff.wildfire.up&cooldown.fire_ball.charges>=1&cooldown.fire_ball.remains<24
actions+=/weapon_arcane_channel,if=cooldown.wildfire.remains>0&!buff.wildfire.up

# Nature's Fury is pure damage in this scenario, so use it whenever the burn setup is not taking priority.
actions+=/weapon_chain_lightning

# Maintain Searing Blaze and use spare Fire Ball charges in cleave.
actions+=/searing_blaze,if=dot.searing_blaze.remains<3
actions+=/fire_ball,if=targets.count>=2&cooldown.fire_ball.charges=2
actions+=/detonate,if=resource.embers.current=resource.embers.max

# Primary filler and Ember generator.
actions+=/infernal_wave
