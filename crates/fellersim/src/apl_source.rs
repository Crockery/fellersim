//! Parser for the documented actions=/... APL language.
use crate::{ActionPriorityListV2, SimulationError, SimulationErrorCode};
use serde_json::{Value, json};

fn error(line: usize, column: usize, message: impl std::fmt::Display) -> SimulationError {
    SimulationError {
        code: SimulationErrorCode::InvalidActionPriorityList,
        message: format!("APL line {line}, column {column}: {message}"),
        sources: vec![],
    }
}

struct Parser {
    tokens: Vec<(String, usize)>,
    index: usize,
    line: usize,
    nodes: usize,
}
impl Parser {
    fn new(source: &str, line: usize, column: usize) -> Result<Self, SimulationError> {
        let mut tokens = Vec::new();
        let mut chars = source.char_indices().peekable();
        while let Some((offset, ch)) = chars.next() {
            if ch.is_whitespace() {
                continue;
            }
            let mut token = ch.to_string();
            if "()!&|=<>".contains(ch) {
                if "!<>".contains(ch) && chars.peek().is_some_and(|(_, c)| *c == '=') {
                    chars.next();
                    token.push('=');
                }
            } else if ch.is_ascii_alphabetic() || ch == '_' {
                while chars
                    .peek()
                    .is_some_and(|(_, c)| c.is_ascii_alphanumeric() || "_.-".contains(*c))
                {
                    token.push(chars.next().unwrap().1);
                }
            } else if ch.is_ascii_digit() || "+-.".contains(ch) {
                while let Some((_, c)) = chars.peek() {
                    if c.is_ascii_digit()
                        || ".eE".contains(*c)
                        || ("+-".contains(*c) && token.ends_with(['e', 'E']))
                    {
                        token.push(chars.next().unwrap().1);
                    } else {
                        break;
                    }
                }
            } else {
                return Err(error(
                    line,
                    column + offset,
                    format!("Unsupported token {ch:?}"),
                ));
            }
            tokens.push((token.to_ascii_lowercase(), column + offset));
            if tokens.len() > 4096 {
                return Err(error(line, column, "Condition is too large"));
            }
        }
        tokens.push((String::new(), column + source.len()));
        Ok(Self {
            tokens,
            index: 0,
            line,
            nodes: 0,
        })
    }
    fn peek(&self) -> &str {
        &self.tokens[self.index.min(self.tokens.len() - 1)].0
    }
    fn fail(&self, message: impl std::fmt::Display) -> SimulationError {
        error(
            self.line,
            self.tokens[self.index.min(self.tokens.len() - 1)].1,
            message,
        )
    }
    fn take(&mut self) -> String {
        let t = self.peek().to_owned();
        self.index += 1;
        t
    }
    fn node(&mut self, mut value: Value) -> Result<Value, SimulationError> {
        self.nodes += 1;
        if self.nodes > 1024 {
            return Err(self.fail("Too many condition nodes"));
        }
        value["id"] = json!(format!("{}:{}", self.line, self.nodes));
        Ok(value)
    }
    fn group(&mut self, and: bool, depth: usize) -> Result<Value, SimulationError> {
        if depth > 64 {
            return Err(self.fail("Condition is nested too deeply"));
        }
        let mut children = vec![if and {
            self.unary(depth)?
        } else {
            self.group(true, depth)?
        }];
        while self.peek() == if and { "&" } else { "|" } {
            self.take();
            children.push(if and {
                self.unary(depth)?
            } else {
                self.group(true, depth)?
            });
        }
        if children.len() == 1 {
            Ok(children.remove(0))
        } else {
            self.node(json!({"kind": if and {"all"} else {"any"}, "children":children}))
        }
    }
    fn unary(&mut self, depth: usize) -> Result<Value, SimulationError> {
        if depth > 64 {
            return Err(self.fail("Condition is nested too deeply"));
        }
        if self.peek() == "!" {
            self.take();
            let child = self.unary(depth + 1)?;
            return self.node(json!({"kind":"not", "child":child}));
        }
        if self.peek() == "(" {
            self.take();
            let value = self.group(false, depth + 1)?;
            if self.take() != ")" {
                return Err(self.fail("Expected closing parenthesis"));
            }
            return Ok(value);
        }
        if let Some(reference) = reference(self.peek(), true) {
            self.take();
            return self.node(json!({"kind":"boolean-reference","reference":reference}));
        }
        let left = self.operand()?;
        let operator = match self.take().as_str() {
            "=" => "equal",
            "!=" => "not-equal",
            "<" => "less-than",
            "<=" => "less-than-or-equal",
            ">" => "greater-than",
            ">=" => "greater-than-or-equal",
            _ => return Err(self.fail("Expected a numeric comparison operator")),
        };
        let right = self.operand()?;
        self.node(json!({"kind":"comparison","left":left,"operator":operator,"right":right}))
    }
    fn operand(&mut self) -> Result<Value, SimulationError> {
        let token = self.take();
        if let Ok(value) = token.parse::<f64>() {
            if !value.is_finite() {
                return Err(self.fail("Use a finite number"));
            }
            return Ok(json!({"kind":"number","value":value}));
        }
        reference(&token, false)
            .map(|r| json!({"kind":"reference","reference":r}))
            .ok_or_else(|| self.fail(format!("Unsupported state query {token:?}")))
    }
}

fn reference(token: &str, boolean: bool) -> Option<Value> {
    let parts: Vec<_> = token.split('.').collect();
    let fields = if parts.len() == 3 {
        let id = parts[1].replace('_', "-");
        match (boolean, parts[0], parts[2]) {
            (true, "cooldown", "ready") => Some(json!({"kind":"cooldown-ready","abilityId":id})),
            (true, "dot", "up") => Some(json!({"kind":"dot-active","abilityId":id})),
            (true, "debuff", "up") => Some(json!({"kind":"target-effect-active","effectId":id})),
            (true, "buff", "up") => Some(json!({"kind":"buff-active","buff":id})),
            (true, "legendary", "equipped") => {
                Some(json!({"kind":"legendary-equipped","itemId":format!("legendary-{id}")}))
            }
            (true, "talent", "enabled") => {
                Some(json!({"kind":"talent-selected","talentId":parts[1]}))
            }
            (false, "cooldown", "remains") => {
                Some(json!({"kind":"cooldown-remaining","abilityId":id}))
            }
            (false, "cooldown", "charges") => {
                Some(json!({"kind":"cooldown-charges","abilityId":id}))
            }
            (false, "dot", "remains") => Some(json!({"kind":"dot-remaining","abilityId":id})),
            (false, "debuff", "remains") => {
                Some(json!({"kind":"target-effect-remaining","effectId":id}))
            }
            (false, "debuff", "stacks") => {
                Some(json!({"kind":"target-effect-stacks","effectId":id}))
            }
            (false, "buff", "remains") => Some(json!({"kind":"buff-remaining","buff":id})),
            (false, "buff", "stacks") => Some(json!({"kind":"buff-stacks","buff":id})),
            (false, "resource", measure) => {
                Some(json!({"kind":"resource","resource":id,"measure":measure}))
            }
            _ => None,
        }
    } else {
        None
    };
    fields.or_else(|| {
        if boolean {
            return None;
        }
        let kind = match token {
            "target.health.pct" => "target-health-percent",
            "target.time_to_die" => "target-time-to-die",
            "targets.count" => "target-count",
            "fight.elapsed" => "fight-elapsed",
            "fight.remains" => "fight-remaining",
            _ => return None,
        };
        Some(json!({"kind":kind}))
    })
}

pub fn parse_apl(source: &str) -> Result<ActionPriorityListV2, SimulationError> {
    if source.len() > 1024 * 1024 {
        return Err(error(1, 1, "APL exceeds 1 MiB"));
    }
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut rules = Vec::new();
    let mut comments = Vec::new();
    for (index, raw) in normalized.lines().enumerate() {
        let line = index + 1;
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let (enabled, text) = match text.strip_prefix('#') {
            Some(comment)
                if comment
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("actions")
                    && comment.contains("=/") =>
            {
                (false, comment.trim_start())
            }
            Some(comment) => {
                comments.push(comment.trim_start().to_owned());
                continue;
            }
            None => (true, text),
        };
        let (code, inline) = text.split_once('#').map_or((text, None), |(c, m)| {
            (c, Some(m.trim()).filter(|s| !s.is_empty()))
        });
        let code = code.trim();
        let (action, condition) = code
            .to_ascii_lowercase()
            .find(",if=")
            .map_or((code, None), |offset| {
                (&code[..offset], Some(&code[offset + 4..]))
            });
        let prefix = if rules.is_empty() {
            "actions=/"
        } else {
            "actions+=/"
        };
        let lower = action.to_ascii_lowercase();
        let ability = lower
            .strip_prefix(prefix)
            .ok_or_else(|| error(line, 1, format!("Expected {prefix}spell[,if=condition]")))?;
        if ability.is_empty()
            || !ability
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
        {
            return Err(error(line, 1, "Invalid action name or condition syntax"));
        }
        let condition = if let Some(source) = condition {
            let mut parser = Parser::new(
                source,
                line,
                raw.to_ascii_lowercase().find(",if=").unwrap_or(0) + 5,
            )?;
            let value = parser.group(false, 0)?;
            if !parser.peek().is_empty() {
                return Err(parser.fail("Unexpected token after condition"));
            }
            // Deserialize here so enum errors also retain the source line.
            serde_json::from_value::<crate::AplExpressionNode>(value.clone())
                .map_err(|e| error(line, 1, e))?;
            value
        } else {
            Value::Null
        };
        rules.push(json!({"id":format!("rule-{}", rules.len()),"enabled":enabled,"abilityId":ability.replace('_',"-"),
            "leadingComments":std::mem::take(&mut comments),"inlineComment":inline,"condition":condition}));
        if rules.len() > 128 {
            return Err(error(line, 1, "An APL supports at most 128 rules"));
        }
    }
    serde_json::from_value(json!({"schemaVersion":2,"rules":rules,"trailingComments":comments}))
        .map_err(|e| error(1, 1, e))
}
