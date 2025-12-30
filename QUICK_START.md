# 🚀 Quick Start Guide - Amigo Oculto

## Setup Rápido para Desenvolvimento

### 1. Configure o Backend

```bash
cd backend
cp .env.example .env
```

Edite o arquivo `.env` com suas configurações de email:

```env
DATABASE_URL=sqlite://data/amigo_oculto.db
PORT=3000
BASE_URL=http://localhost:3000

# Gmail - Use uma senha de app específica
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=seu-email@gmail.com
SMTP_PASSWORD=sua-senha-de-app
SMTP_FROM=noreply@amigooculto.app
```

**Como criar uma senha de app no Gmail:**
1. Vá em https://myaccount.google.com
2. Segurança → Verificação em duas etapas (ative se necessário)
3. Role até "Senhas de app"
4. Crie uma nova senha de app para "Mail"
5. Use essa senha no arquivo `.env`

### 2. Inicie o Backend

```bash
# Criar diretório para banco de dados
mkdir -p data

# Executar o servidor
cargo run
```

O backend estará rodando em `http://localhost:3000`

### 3. Configure o Frontend (Novo Terminal)

```bash
cd frontend
npm install
```

### 4. Inicie o Frontend

```bash
npm run dev
```

O frontend estará rodando em `http://localhost:5173`

## ✅ Teste a Aplicação

1. Abra `http://localhost:5173` no navegador
2. Clique em "Criar Novo Jogo"
3. Preencha:
   - Nome do Jogo: "Teste"
   - Data do Evento: "Hoje"
   - Seu Email: seu email real
4. Adicione pelo menos 2 participantes com emails reais
5. Clique em "Realizar Sorteio e Enviar Emails"
6. Verifique os emails dos participantes!

## 📦 Build para Produção

### Backend
```bash
cd backend
cargo build --release
# O binário estará em: target/release/amigo-oculto-backend
```

### Frontend
```bash
cd frontend
npm run build
# Os arquivos estáticos estarão em: build/
```

### Com Docker
```bash
# Build do frontend primeiro
cd frontend
npm install
npm run build
cd ..

# Configure o .env
cp backend/.env.example backend/.env
# Edite backend/.env com suas configurações

# Inicie com Docker
docker-compose up -d
```

Acesse: `http://localhost:3000`

## 🐛 Problemas Comuns

### "Failed to send email"
- Verifique se o SMTP_PASSWORD está correto (use App Password do Gmail)
- Teste outro provedor SMTP
- Verifique os logs: `cargo run` ou `docker-compose logs -f`

### "Database locked"
- Pare o servidor e delete `data/amigo_oculto.db`
- Execute novamente

### Frontend não conecta à API
- Certifique-se que o backend está rodando
- Verifique se a porta 3000 está livre
- Em desenvolvimento, o proxy do Vite deve funcionar automaticamente

## 📝 Próximos Passos

1. Personalize o `BASE_URL` no `.env` para seu domínio
2. Configure HTTPS se for usar em produção
3. Faça backup regular do banco de dados em `data/`
4. Considere usar um serviço de email transacional (SendGrid, Mailgun, etc.)

## 💡 Dicas

- Use o mesmo email de teste para organizador e participantes durante os testes
- O organizador recebe um link de administração por email
- Cada participante recebe um link único para ver quem tirou
- Links são válidos indefinidamente e podem ser reutilizados
- O banco de dados SQLite é apenas um arquivo - fácil de fazer backup!

---

Pronto! Agora você tem um sistema completo de Amigo Oculto funcionando! 🎉