use crate::types::{FuncId, Prefix, PrefixConfig, PrefixSignature, WorkflowId};

pub fn make_prefix_signature(
    workflow_id: &WorkflowId,
    prefix: &Prefix,
    config: PrefixConfig,
) -> PrefixSignature {
    let funcs = last_n(&prefix.funcs, config.lmax);
    let mut out = String::with_capacity(workflow_id.0.len() + 1 + funcs.len() * 8);
    out.push_str(&workflow_id.0);
    out.push('\0');
    for (i, f) in funcs.iter().enumerate() {
        if i > 0 {
            out.push('→');
        }
        out.push_str(&f.0);
    }
    PrefixSignature(out)
}

pub fn parse_prefix_signature(signature: &PrefixSignature) -> Option<(WorkflowId, Vec<FuncId>)> {
    let mut parts = signature.0.splitn(2, '\0');
    let workflow = parts.next()?;
    let tail = parts.next().unwrap_or_default();
    let funcs = if tail.is_empty() {
        Vec::new()
    } else {
        tail.split('→').map(FuncId::new).collect()
    };
    Some((WorkflowId::new(workflow), funcs))
}

fn last_n<T: Clone>(items: &[T], n: usize) -> Vec<T> {
    if n == 0 {
        return Vec::new();
    }
    if items.len() <= n {
        return items.to_vec();
    }
    items[items.len() - n..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_signature_with_lmax_tail() {
        let workflow_id = WorkflowId::from("w1");
        let prefix = Prefix::new(vec![
            FuncId::from("A"),
            FuncId::from("B"),
            FuncId::from("C"),
        ]);
        let sig = make_prefix_signature(&workflow_id, &prefix, PrefixConfig { lmax: 2 });
        assert_eq!(sig.0, "w1\0B→C");
    }

    #[test]
    fn parses_signature_roundtrip() {
        let sig = PrefixSignature::from("w1\0A→B");
        let (w, funcs) = parse_prefix_signature(&sig).unwrap();
        assert_eq!(w, WorkflowId::from("w1"));
        assert_eq!(funcs, vec![FuncId::from("A"), FuncId::from("B")]);
    }
}

