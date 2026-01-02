/// CSS styles for email templates
/// Color palette: charcoal (#4A5759), sage (#B0C4B1), cream (#F7E1D7), sage-light (#DEDBD2), blush (#EDAFB8)
pub const EMAIL_STYLES: &str = r#"
body {
    font-family: Arial, sans-serif;
    line-height: 1.6;
    color: #4A5759;
    max-width: 600px;
    margin: 0 auto;
    padding: 20px;
}

.header {
    background: #4A5759;
    color: white;
    padding: 30px;
    border-radius: 10px 10px 0 0;
    text-align: center;
}

.header h1 {
    margin: 0;
    font-size: 28px;
}

.header p {
    margin: 10px 0 0 0;
    font-size: 18px;
}

.content {
    background: #F7E1D7;
    padding: 30px;
    border-radius: 0 0 10px 10px;
    border: 1px solid #DEDBD2;
    border-top: none;
}

.content p {
    font-size: 16px;
}

.info-box {
    background: white;
    padding: 20px;
    border-radius: 8px;
    margin: 25px 0;
    border-left: 4px solid #4A5759;
}

.success-box {
    background: white;
    padding: 20px;
    border-radius: 8px;
    margin: 20px 0;
    border-left: 4px solid #B0C4B1;
}

.warning-box {
    background: #fff3cd;
    padding: 15px;
    border-radius: 8px;
    margin: 20px 0;
    border-left: 4px solid #ffc107;
}

.warning-box p {
    font-size: 14px;
    margin: 0;
    color: #856404;
}

.btn {
    display: inline-block;
    background: #4A5759;
    color: white;
    padding: 15px 40px;
    text-decoration: none;
    border-radius: 5px;
    font-size: 18px;
    font-weight: bold;
}

.text-center {
    text-align: center;
}

.text-muted {
    font-size: 14px;
    color: #697478;
}

.footer {
    text-align: center;
    margin-top: 20px;
    padding: 20px;
    color: #879093;
    font-size: 12px;
}

.footer p {
    margin: 0;
}

a {
    color: #4A5759;
    word-break: break-all;
}
"#;
