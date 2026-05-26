use crate::diagnostics::ParseDiagnostic;
use crate::{ParsedExpr, eval_with_vars, parse_curve_expr, parse_expr_with_vars};

fn merge_parameters<'a>(parts: impl IntoIterator<Item = &'a ParsedExpr>) -> Vec<(String, f64)> {
    let mut merged = Vec::new();
    for parsed in parts {
        for (name, default) in &parsed.parameters {
            if !merged.iter().any(|(existing, _)| existing == name) {
                merged.push((name.clone(), *default));
            }
        }
    }
    merged
}

fn parse_pipe_components(
    source: &str,
    coord_vars: &[&str],
    labels: [&str; 3],
    context: &str,
) -> Result<[ParsedExpr; 3], ParseDiagnostic> {
    let parts: Vec<&str> = source.splitn(3, '|').collect();
    if parts.len() != 3 {
        return Err(ParseDiagnostic::new(format!(
            "{context} must contain {}|{}|{}",
            labels[0], labels[1], labels[2]
        )));
    }
    let first = parse_expr_with_vars(parts[0], coord_vars)
        .map_err(|err| ParseDiagnostic::new(err).with_component(labels[0]))?;
    let second = parse_expr_with_vars(parts[1], coord_vars)
        .map_err(|err| ParseDiagnostic::new(err).with_component(labels[1]))?;
    let third = parse_expr_with_vars(parts[2], coord_vars)
        .map_err(|err| ParseDiagnostic::new(err).with_component(labels[2]))?;
    Ok([first, second, third])
}

#[derive(Clone)]
pub struct ScalarFieldExpr {
    source: String,
    coord_vars: Vec<String>,
    parsed: ParsedExpr,
}

impl ScalarFieldExpr {
    pub fn parse(source: &str, coord_vars: &[&str]) -> Result<Self, ParseDiagnostic> {
        let parsed = parse_expr_with_vars(source, coord_vars).map_err(ParseDiagnostic::new)?;
        Ok(Self {
            source: source.to_string(),
            coord_vars: coord_vars
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            parsed,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn coord_vars(&self) -> &[String] {
        &self.coord_vars
    }

    pub fn parsed(&self) -> &ParsedExpr {
        &self.parsed
    }

    pub fn parameters(&self) -> &[(String, f64)] {
        &self.parsed.parameters
    }

    pub fn eval(&self, vars: &[(&str, f64)]) -> f64 {
        eval_with_vars(&self.parsed, vars)
    }
}

#[derive(Clone)]
pub struct VectorFieldExpr {
    source: String,
    coord_vars: Vec<String>,
    components: [ParsedExpr; 3],
    parameters: Vec<(String, f64)>,
}

impl VectorFieldExpr {
    pub fn parse(source: &str, coord_vars: &[&str]) -> Result<Self, ParseDiagnostic> {
        let components = parse_pipe_components(
            source,
            coord_vars,
            ["vx", "vy", "vz"],
            "vector field expression",
        )?;
        let parameters = merge_parameters(components.iter());
        Ok(Self {
            source: source.to_string(),
            coord_vars: coord_vars
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            components,
            parameters,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn coord_vars(&self) -> &[String] {
        &self.coord_vars
    }

    pub fn components(&self) -> &[ParsedExpr; 3] {
        &self.components
    }

    pub fn parameters(&self) -> &[(String, f64)] {
        &self.parameters
    }
}

#[derive(Clone)]
pub struct ParametricSurfaceExpr {
    source: String,
    components: [ParsedExpr; 3],
    parameters: Vec<(String, f64)>,
}

impl ParametricSurfaceExpr {
    pub fn parse(source: &str) -> Result<Self, ParseDiagnostic> {
        let components = parse_pipe_components(
            source,
            &["u", "v"],
            ["x", "y", "z"],
            "parametric surface expression",
        )?;
        let parameters = merge_parameters(components.iter());
        Ok(Self {
            source: source.to_string(),
            components,
            parameters,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn components(&self) -> &[ParsedExpr; 3] {
        &self.components
    }

    pub fn parameters(&self) -> &[(String, f64)] {
        &self.parameters
    }
}

#[derive(Clone)]
pub struct ParametricCurveExpr {
    source: String,
    components: (ParsedExpr, ParsedExpr, ParsedExpr),
    parameters: Vec<(String, f64)>,
}

impl ParametricCurveExpr {
    pub fn parse(source: &str) -> Result<Self, ParseDiagnostic> {
        let (x, y, z) = parse_curve_expr(source).map_err(ParseDiagnostic::new)?;
        let parameters = merge_parameters([&x, &y, &z]);
        Ok(Self {
            source: source.to_string(),
            components: (x, y, z),
            parameters,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn components(&self) -> &(ParsedExpr, ParsedExpr, ParsedExpr) {
        &self.components
    }

    pub fn parameters(&self) -> &[(String, f64)] {
        &self.parameters
    }
}
