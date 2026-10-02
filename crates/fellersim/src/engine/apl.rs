use crate::*;

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AplNodeEvaluationTrace {
    pub(crate) node_id: String,
    pub(crate) passed: bool,
    pub(crate) observed_values: Vec<f64>,
    pub(crate) short_circuited: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AplRuleEvaluationTrace {
    pub(crate) rule_id: String,
    pub(crate) passed: bool,
    pub(crate) castable: bool,
    pub(crate) nodes: Vec<AplNodeEvaluationTrace>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeAplRule {
    pub(crate) ability_kind: DpsAbilityKind,
    pub(crate) condition: Option<AplExpressionNode>,
    pub(crate) active_from_ms: u64,
    pub(crate) active_until_ms: u64,
}

enum FoldedAplExpression {
    Static(bool),
    Dynamic(AplExpressionNode),
}

pub(crate) fn compile_runtime_apl(
    apl: &ActionPriorityListV2,
    profile: &CompiledProfile,
    target_count: u32,
) -> (Vec<RuntimeAplRule>, usize) {
    let mut work = apl.rules.len();
    let rules = apl
        .rules
        .iter()
        .filter_map(|rule| {
            if !rule.enabled {
                return None;
            }
            let ability_kind = ability_kind_from_id(&rule.ability_id)?;
            // Default rotations list alternative weapons and optional skills.
            // The immutable profile cannot acquire a missing ability mid-run.
            profile.ability(ability_kind)?;
            let condition = match rule.condition.as_ref() {
                None => None,
                Some(condition) => {
                    match fold_apl_expression(condition, profile, target_count, &mut work) {
                        FoldedAplExpression::Static(false) => return None,
                        FoldedAplExpression::Static(true) => None,
                        FoldedAplExpression::Dynamic(condition) => Some(condition),
                    }
                }
            };
            let fight_window = condition.as_ref().and_then(extract_fight_window);
            let (condition, active_from_ms, active_until_ms) = fight_window
                .map_or((condition, 0, u64::MAX), |window| {
                    (None, window.0, window.1)
                });
            Some(RuntimeAplRule {
                ability_kind,
                condition,
                active_from_ms,
                active_until_ms,
            })
        })
        .collect();
    (rules, work)
}

fn extract_fight_window(node: &AplExpressionNode) -> Option<(u64, u64)> {
    let AplExpression::All { children } = &node.expression else {
        return None;
    };
    if children.len() != 2 {
        return None;
    }
    let mut start = None;
    let mut end = None;
    for child in children {
        let AplExpression::Comparison {
            left:
                AplNumericOperand::Reference {
                    reference: AplNumericReference::FightElapsed,
                },
            operator,
            right: AplNumericOperand::Number { value },
        } = &child.expression
        else {
            return None;
        };
        match operator {
            AplComparisonOperator::GreaterThanOrEqual if start.is_none() => {
                start = Some((*value * 1_000.0).ceil().max(0.0) as u64);
            }
            AplComparisonOperator::LessThan if end.is_none() => {
                end = Some((*value * 1_000.0).ceil().max(0.0) as u64);
            }
            _ => return None,
        }
    }
    Some((start?, end?))
}

fn fold_apl_expression(
    node: &AplExpressionNode,
    profile: &CompiledProfile,
    target_count: u32,
    work: &mut usize,
) -> FoldedAplExpression {
    *work = work.saturating_add(1);
    let expression = match &node.expression {
        AplExpression::All { children } => {
            let mut dynamic = Vec::new();
            for child in children {
                match fold_apl_expression(child, profile, target_count, work) {
                    FoldedAplExpression::Static(false) => {
                        return FoldedAplExpression::Static(false);
                    }
                    FoldedAplExpression::Static(true) => {}
                    FoldedAplExpression::Dynamic(child) => match child.expression {
                        AplExpression::All { children } => dynamic.extend(children),
                        expression => dynamic.push(AplExpressionNode {
                            id: child.id,
                            expression,
                        }),
                    },
                }
            }
            if dynamic.is_empty() {
                return FoldedAplExpression::Static(true);
            }
            if dynamic.len() == 1 {
                return FoldedAplExpression::Dynamic(dynamic.pop().expect("one dynamic APL child"));
            }
            AplExpression::All { children: dynamic }
        }
        AplExpression::Any { children } => {
            let mut dynamic = Vec::new();
            for child in children {
                match fold_apl_expression(child, profile, target_count, work) {
                    FoldedAplExpression::Static(true) => {
                        return FoldedAplExpression::Static(true);
                    }
                    FoldedAplExpression::Static(false) => {}
                    FoldedAplExpression::Dynamic(child) => match child.expression {
                        AplExpression::Any { children } => dynamic.extend(children),
                        expression => dynamic.push(AplExpressionNode {
                            id: child.id,
                            expression,
                        }),
                    },
                }
            }
            if dynamic.is_empty() {
                return FoldedAplExpression::Static(false);
            }
            if dynamic.len() == 1 {
                return FoldedAplExpression::Dynamic(dynamic.pop().expect("one dynamic APL child"));
            }
            AplExpression::Any { children: dynamic }
        }
        AplExpression::Not { child } => {
            match fold_apl_expression(child, profile, target_count, work) {
                FoldedAplExpression::Static(value) => {
                    return FoldedAplExpression::Static(!value);
                }
                FoldedAplExpression::Dynamic(child) => AplExpression::Not {
                    child: Box::new(child),
                },
            }
        }
        AplExpression::BooleanReference {
            reference: AplBooleanReference::TalentSelected { talent_id },
        } => {
            return FoldedAplExpression::Static(profile.talents_by_id.contains_key(talent_id));
        }
        AplExpression::BooleanReference {
            reference: AplBooleanReference::LegendaryEquipped { item_id },
        } => {
            return FoldedAplExpression::Static(
                profile
                    .mechanics
                    .iter()
                    .any(|mechanic| mechanic.source.source_id == *item_id),
            );
        }
        AplExpression::Comparison {
            left,
            operator,
            right,
        } => {
            let left_static = static_numeric_operand(left, target_count);
            let right_static = static_numeric_operand(right, target_count);
            if let (Some(left), Some(right)) = (left_static, right_static) {
                return FoldedAplExpression::Static(compare_apl_values(left, *operator, right));
            }
            node.expression.clone()
        }
        AplExpression::BooleanReference { .. } => node.expression.clone(),
    };
    FoldedAplExpression::Dynamic(AplExpressionNode {
        id: node.id.clone(),
        expression,
    })
}

fn static_numeric_operand(operand: &AplNumericOperand, target_count: u32) -> Option<f64> {
    match operand {
        AplNumericOperand::Number { value } => Some(*value),
        AplNumericOperand::Reference {
            reference: AplNumericReference::TargetCount,
        } => Some(f64::from(target_count)),
        AplNumericOperand::Reference { .. } => None,
    }
}

fn compare_apl_values(left: f64, operator: AplComparisonOperator, right: f64) -> bool {
    match operator {
        AplComparisonOperator::Equal => left == right,
        AplComparisonOperator::NotEqual => left != right,
        AplComparisonOperator::LessThan => left < right,
        AplComparisonOperator::LessThanOrEqual => left <= right,
        AplComparisonOperator::GreaterThan => left > right,
        AplComparisonOperator::GreaterThanOrEqual => left >= right,
    }
}

impl Iteration<'_> {
    pub(crate) fn choose_action(&self) -> Option<usize> {
        let mut pending_rule_scans = 0_u8;
        for rule in &self.common.runtime_action_priority_list {
            pending_rule_scans += 1;
            if pending_rule_scans == 8 {
                if !self.common.execution.charge(1) {
                    return None;
                }
                pending_rule_scans = 0;
            }
            if self.common.now_ms < rule.active_from_ms
                || self.common.now_ms >= rule.active_until_ms
            {
                continue;
            }
            if !self.common.execution.charge(1) {
                return None;
            }
            // Conditions are pure observations. Resolve availability first so
            // cooling-down or resource-blocked abilities do not repeatedly walk
            // their expression trees after every periodic damage event.
            let Some(cast) = self.can_cast(rule.ability_kind) else {
                continue;
            };
            let passed = rule
                .condition
                .as_ref()
                .is_none_or(|condition| self.evaluate_expression_untraced(condition));
            if !passed {
                continue;
            }
            if pending_rule_scans > 0 && !self.common.execution.charge(1) {
                return None;
            }
            return Some(cast);
        }
        if pending_rule_scans > 0 {
            self.common.execution.charge(1);
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn choose_action_with_trace(
        &self,
        mut trace: Option<&mut Vec<AplRuleEvaluationTrace>>,
    ) -> Option<usize> {
        for rule in &self.common.action_priority_list.rules {
            if !self.common.execution.charge(1) {
                return None;
            }
            if !rule.enabled {
                continue;
            }
            let mut nodes = Vec::new();
            let passed = rule
                .condition
                .as_ref()
                .is_none_or(|condition| self.evaluate_expression_traced(condition, &mut nodes));
            let cast = passed
                .then(|| ability_kind_from_id(&rule.ability_id))
                .flatten()
                .and_then(|kind| self.can_cast(kind));
            if let Some(trace) = trace.as_deref_mut() {
                trace.push(AplRuleEvaluationTrace {
                    rule_id: rule.id.clone(),
                    passed,
                    castable: cast.is_some(),
                    nodes,
                });
            }
            if cast.is_some() {
                return cast;
            }
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn evaluate_expression(&self, node: &AplExpressionNode) -> bool {
        self.evaluate_expression_traced(node, &mut Vec::new())
    }

    #[cfg(test)]
    fn evaluate_expression_traced(
        &self,
        node: &AplExpressionNode,
        trace: &mut Vec<AplNodeEvaluationTrace>,
    ) -> bool {
        if !self.common.execution.charge(1) {
            return false;
        }
        let trace_index = trace.len();
        trace.push(AplNodeEvaluationTrace {
            node_id: node.id.clone(),
            passed: false,
            observed_values: Vec::new(),
            short_circuited: false,
        });
        let result = match &node.expression {
            AplExpression::All { children } => {
                let mut passed = true;
                for (index, child) in children.iter().enumerate() {
                    if !self.evaluate_expression_traced(child, trace) {
                        passed = false;
                        self.record_short_circuited(&children[index + 1..], trace);
                        break;
                    }
                }
                passed
            }
            AplExpression::Any { children } => {
                let mut passed = false;
                for (index, child) in children.iter().enumerate() {
                    if self.evaluate_expression_traced(child, trace) {
                        passed = true;
                        self.record_short_circuited(&children[index + 1..], trace);
                        break;
                    }
                }
                passed
            }
            AplExpression::Not { child } => !self.evaluate_expression_traced(child, trace),
            AplExpression::BooleanReference { reference } => {
                self.evaluate_boolean_reference(reference)
            }
            AplExpression::Comparison {
                left,
                operator,
                right,
            } => {
                let left = self.evaluate_numeric_operand(left);
                let right = self.evaluate_numeric_operand(right);
                trace[trace_index].observed_values = vec![left, right];
                match operator {
                    AplComparisonOperator::Equal => left == right,
                    AplComparisonOperator::NotEqual => left != right,
                    AplComparisonOperator::LessThan => left < right,
                    AplComparisonOperator::LessThanOrEqual => left <= right,
                    AplComparisonOperator::GreaterThan => left > right,
                    AplComparisonOperator::GreaterThanOrEqual => left >= right,
                }
            }
        };
        trace[trace_index].passed = result;
        result
    }

    fn evaluate_expression_untraced(&self, node: &AplExpressionNode) -> bool {
        if !self.common.execution.charge(1) {
            return false;
        }
        match &node.expression {
            AplExpression::All { children } => children
                .iter()
                .all(|child| self.evaluate_expression_untraced(child)),
            AplExpression::Any { children } => children
                .iter()
                .any(|child| self.evaluate_expression_untraced(child)),
            AplExpression::Not { child } => !self.evaluate_expression_untraced(child),
            AplExpression::BooleanReference { reference } => {
                self.evaluate_boolean_reference(reference)
            }
            AplExpression::Comparison {
                left,
                operator,
                right,
            } => compare_apl_values(
                self.evaluate_numeric_operand(left),
                *operator,
                self.evaluate_numeric_operand(right),
            ),
        }
    }

    #[cfg(test)]
    fn record_short_circuited(
        &self,
        children: &[AplExpressionNode],
        trace: &mut Vec<AplNodeEvaluationTrace>,
    ) {
        for child in children {
            self.record_short_circuited_node(child, trace);
        }
    }

    #[cfg(test)]
    fn record_short_circuited_node(
        &self,
        node: &AplExpressionNode,
        trace: &mut Vec<AplNodeEvaluationTrace>,
    ) {
        trace.push(AplNodeEvaluationTrace {
            node_id: node.id.clone(),
            passed: false,
            observed_values: Vec::new(),
            short_circuited: true,
        });
        match &node.expression {
            AplExpression::All { children } | AplExpression::Any { children } => {
                self.record_short_circuited(children, trace);
            }
            AplExpression::Not { child } => self.record_short_circuited_node(child, trace),
            AplExpression::BooleanReference { .. } | AplExpression::Comparison { .. } => {}
        }
    }

    fn evaluate_boolean_reference(&self, reference: &AplBooleanReference) -> bool {
        match reference {
            AplBooleanReference::CooldownReady { ability_id } => ability_kind_from_id(ability_id)
                .filter(|kind| self.ability(*kind).is_some())
                .is_some_and(|kind| self.cooldown_remaining_ms(kind) == 0),
            AplBooleanReference::DotActive { ability_id } => {
                ability_kind_from_id(ability_id).is_some_and(|kind| self.dot_remaining(kind) > 0)
            }
            AplBooleanReference::BuffActive { buff } => self.buff_stacks(*buff) > 0,
            AplBooleanReference::TargetEffectActive { effect_id } => self
                .target_effect_state(effect_id)
                .is_some_and(|(remaining, stacks)| remaining > 0 && stacks > 0),
            AplBooleanReference::LegendaryEquipped { item_id } => self
                .profile
                .mechanics
                .iter()
                .any(|mechanic| mechanic.source.source_id == *item_id),
            AplBooleanReference::TalentSelected { talent_id } => self
                .common
                .selected_talents
                .contains_key(talent_id.as_str()),
        }
    }

    pub(crate) fn evaluate_numeric_operand(&self, operand: &AplNumericOperand) -> f64 {
        match operand {
            AplNumericOperand::Number { value } => *value,
            AplNumericOperand::Reference { reference } => {
                self.evaluate_numeric_reference(reference)
            }
        }
    }

    pub(crate) fn evaluate_numeric_reference(&self, reference: &AplNumericReference) -> f64 {
        match reference {
            AplNumericReference::CooldownRemaining { ability_id } => {
                ability_kind_from_id(ability_id)
                    .map(|kind| self.cooldown_remaining_ms(kind) as f64 / 1_000.0)
                    .unwrap_or(0.0)
            }
            AplNumericReference::CooldownCharges { ability_id } => ability_kind_from_id(ability_id)
                .and_then(|kind| self.ability(kind).map(|ability| (kind, ability)))
                .map(|(kind, ability)| {
                    let used = self
                        .common
                        .cooldowns
                        .get(&kind)
                        .map(|cooldown| cooldown.used_charges)
                        .unwrap_or(0);
                    ability.maximum_charges.max(1).saturating_sub(used) as f64
                })
                .unwrap_or(0.0),
            AplNumericReference::DotRemaining { ability_id } => ability_kind_from_id(ability_id)
                .map(|kind| self.dot_remaining(kind) as f64 / 1_000.0)
                .unwrap_or(0.0),
            AplNumericReference::Resource { resource, measure } => {
                let (current, maximum) = match resource {
                    AplResource::Cinders => (
                        self.hero.ardeos().cinders,
                        self.profile.max_primary_resource,
                    ),
                    AplResource::Embers => (
                        f64::from(self.hero.ardeos().embers),
                        f64::from(self.profile.max_secondary_resource),
                    ),
                    AplResource::Anima => {
                        (self.hero.rime().anima, self.profile.max_primary_resource)
                    }
                    AplResource::WinterOrbs => (
                        f64::from(self.hero.rime().winter_orbs),
                        f64::from(self.profile.max_secondary_resource),
                    ),
                    AplResource::Fury => {
                        (self.hero.tariq().fury, self.profile.max_primary_resource)
                    }
                    AplResource::Focus => {
                        (self.hero.elarion().focus, self.profile.max_primary_resource)
                    }
                    AplResource::Energy => {
                        (self.hero.mara().energy, self.profile.max_primary_resource)
                    }
                    AplResource::ComboPoints => (
                        f64::from(self.hero.mara().combo_points),
                        f64::from(self.profile.max_secondary_resource),
                    ),
                    AplResource::BloodFeathers => (
                        f64::from(self.gunde_blood_feathers()),
                        f64::from(self.profile.max_secondary_resource),
                    ),
                    AplResource::Spirit => (self.shared.spirit, self.profile.max_spirit),
                };
                match measure {
                    AplResourceMeasure::Current => current,
                    AplResourceMeasure::Max => maximum,
                    AplResourceMeasure::Deficit => (maximum - current).max(0.0),
                    AplResourceMeasure::Percent => {
                        if maximum > 0.0 {
                            current / maximum * 100.0
                        } else {
                            0.0
                        }
                    }
                }
            }
            AplNumericReference::BuffRemaining { buff } => {
                self.buff_remaining_ms(*buff) as f64 / 1_000.0
            }
            AplNumericReference::BuffStacks { buff } => f64::from(self.buff_stacks(*buff)),
            AplNumericReference::TargetEffectRemaining { effect_id } => self
                .target_effect_state(effect_id)
                .map(|(remaining, _)| remaining as f64 / 1_000.0)
                .unwrap_or(0.0),
            AplNumericReference::TargetEffectStacks { effect_id } => self
                .target_effect_state(effect_id)
                .map(|(_, stacks)| f64::from(stacks))
                .unwrap_or(0.0),
            AplNumericReference::TargetCount => f64::from(self.common.target_count),
            AplNumericReference::TargetHealthPercent => {
                ENCOUNTER_DURATION_MS
                    .saturating_sub(self.common.now_ms)
                    .min(ENCOUNTER_DURATION_MS) as f64
                    / ENCOUNTER_DURATION_MS as f64
                    * 100.0
            }
            AplNumericReference::TargetTimeToDie | AplNumericReference::FightRemaining => {
                ENCOUNTER_DURATION_MS.saturating_sub(self.common.now_ms) as f64 / 1_000.0
            }
            AplNumericReference::FightElapsed => self.common.now_ms as f64 / 1_000.0,
        }
    }

    fn target_effect_state(&self, effect_id: &str) -> Option<(u64, u32)> {
        let effect = self
            .profile
            .apl_target_effects
            .iter()
            .find(|effect| effect.id == effect_id)?;
        match &effect.source {
            AplTargetEffectSource::AbilityDot { ability_id } => {
                let kind = ability_kind_from_id(ability_id)?;
                let dot = self.common.dots.get(&(0, DotKind::Ability(kind)))?;
                Some((
                    dot.expires_ms.saturating_sub(self.common.now_ms),
                    dot.stacks,
                ))
            }
            AplTargetEffectSource::MechanicTargetBuff {
                mechanic_instance_id,
            } => {
                let index = self
                    .profile
                    .mechanics
                    .iter()
                    .position(|mechanic| mechanic.instance_id == *mechanic_instance_id)?;
                let slot = self.mechanic_target_slot(index, 0);
                let state = self.shared.dynamic_target_buffs.get(slot)?.as_ref()?;
                Some((
                    state.until_ms.saturating_sub(self.common.now_ms),
                    state.stacks,
                ))
            }
        }
    }
}

pub(crate) fn fight_threshold_times(node: &AplExpressionNode) -> (Vec<u64>, usize) {
    fn threshold(reference: &AplNumericReference, other: &AplNumericOperand) -> Option<u64> {
        let AplNumericOperand::Number { value: seconds } = other else {
            return None;
        };
        if !seconds.is_finite() || *seconds < 0.0 {
            return None;
        }
        let threshold_ms = (*seconds * 1_000.0).ceil();
        if threshold_ms > u64::MAX as f64 {
            return None;
        }
        let threshold_ms = threshold_ms as u64;
        match reference {
            AplNumericReference::FightElapsed => Some(threshold_ms),
            AplNumericReference::FightRemaining | AplNumericReference::TargetTimeToDie => {
                Some(ENCOUNTER_DURATION_MS.saturating_sub(threshold_ms))
            }
            AplNumericReference::TargetHealthPercent => {
                let remaining = (*seconds / 100.0 * ENCOUNTER_DURATION_MS as f64)
                    .round()
                    .clamp(0.0, ENCOUNTER_DURATION_MS as f64)
                    as u64;
                Some(ENCOUNTER_DURATION_MS.saturating_sub(remaining))
            }
            _ => None,
        }
    }

    fn operand_threshold(value: &AplNumericOperand, other: &AplNumericOperand) -> Option<u64> {
        let AplNumericOperand::Reference { reference } = value else {
            return None;
        };
        threshold(reference, other)
    }

    fn collect(node: &AplExpressionNode, times: &mut Vec<u64>, visited: &mut usize) {
        *visited = visited.saturating_add(1);
        match &node.expression {
            AplExpression::All { children } | AplExpression::Any { children } => {
                for child in children {
                    collect(child, times, visited);
                }
            }
            AplExpression::Not { child } => collect(child, times, visited),
            AplExpression::Comparison { left, right, .. } => {
                if let Some(time) =
                    operand_threshold(left, right).or_else(|| operand_threshold(right, left))
                {
                    times.push(time);
                    times.push(time.saturating_add(1));
                }
            }
            AplExpression::BooleanReference { .. } => {}
        }
    }

    let mut times = Vec::new();
    let mut visited = 0;
    collect(node, &mut times, &mut visited);
    (times, visited)
}
