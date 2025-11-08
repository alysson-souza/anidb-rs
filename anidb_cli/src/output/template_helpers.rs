use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderError,
    RenderErrorReason,
};

/// Register custom Handlebars helpers
pub fn register_helpers(handlebars: &mut Handlebars) {
    handlebars.register_helper("format_bytes", Box::new(format_bytes_helper));
    handlebars.register_helper("format_duration", Box::new(format_duration_helper));
    handlebars.register_helper("uppercase", Box::new(uppercase_helper));
    handlebars.register_helper("lowercase", Box::new(lowercase_helper));
    handlebars.register_helper("basename", Box::new(basename_helper));
}

/// Format bytes as human-readable string
fn format_bytes_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "format_bytes expects 1 parameter".into(),
        ))
    })?;

    let bytes = param.value().as_u64().ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "format_bytes expects a number".into(),
        ))
    })?;

    let formatted = crate::progress::format_bytes(bytes);
    out.write(&formatted)?;
    Ok(())
}

/// Format duration in milliseconds as seconds
fn format_duration_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "format_duration expects 1 parameter".into(),
        ))
    })?;

    let millis = param.value().as_u64().ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "format_duration expects a number".into(),
        ))
    })?;

    let seconds = millis as f64 / 1000.0;
    out.write(&format!("{seconds:.2}s"))?;
    Ok(())
}

/// Convert string to uppercase
fn uppercase_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "uppercase expects 1 parameter".into(),
        ))
    })?;

    let text = param.value().as_str().ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "uppercase expects a string".into(),
        ))
    })?;

    out.write(&text.to_uppercase())?;
    Ok(())
}

/// Convert string to lowercase
fn lowercase_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "lowercase expects 1 parameter".into(),
        ))
    })?;

    let text = param.value().as_str().ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "lowercase expects a string".into(),
        ))
    })?;

    out.write(&text.to_lowercase())?;
    Ok(())
}

/// Get basename of a path
fn basename_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other(
            "basename expects 1 parameter".into(),
        ))
    })?;

    let path_str = param.value().as_str().ok_or_else(|| {
        RenderError::from(RenderErrorReason::Other("basename expects a string".into()))
    })?;

    let path = std::path::Path::new(path_str);
    if let Some(basename) = path.file_name() {
        out.write(&basename.to_string_lossy())?;
    }
    Ok(())
}
