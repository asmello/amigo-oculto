/// CSS styles for email templates
pub const EMAIL_STYLES: &str = r#"
body {
    font-family: Arial, sans-serif;
    line-height: 1.6;
    color: #333;
    max-width: 600px;
    margin: 0 auto;
    padding: 20px;
}

.header {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
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
    background: #f9f9f9;
    padding: 30px;
    border-radius: 0 0 10px 10px;
    border: 1px solid #ddd;
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
    border-left: 4px solid #667eea;
}

.success-box {
    background: white;
    padding: 20px;
    border-radius: 8px;
    margin: 20px 0;
    border-left: 4px solid #28a745;
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
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
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
    color: #666;
}

.footer {
    text-align: center;
    margin-top: 20px;
    padding: 20px;
    color: #999;
    font-size: 12px;
}

.footer p {
    margin: 0;
}

a {
    color: #667eea;
    word-break: break-all;
}
"#;