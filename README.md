# 🎁 Amigo Oculto - Sistema de Sorteio Online

[![Deploy to Staging](https://github.com/asmello/amigo-oculto/actions/workflows/deploy-staging.yml/badge.svg)](https://github.com/asmello/amigo-oculto/actions/workflows/deploy-staging.yml)
[![Deploy to Production](https://github.com/asmello/amigo-oculto/actions/workflows/deploy-production.yml/badge.svg)](https://github.com/asmello/amigo-oculto/actions/workflows/deploy-production.yml)

Sistema completo de Amigo Oculto (Secret Santa) localizado para Português Brasileiro, com backend em Rust e frontend em TypeScript/SvelteKit.

> **💡 Nota:** Este projeto foi desenvolvido com a assistência de [Claude Code](https://claude.ai/code), um assistente de programação baseado em IA.

## 🌟 Funcionalidades

- ✅ Criar jogos de Amigo Oculto
- ✅ Adicionar participantes com nome e email
- ✅ Sorteio automático (ninguém tira a si mesmo)
- ✅ Envio de emails automático para cada participante
- ✅ Links únicos para visualizar quem cada pessoa tirou
- ✅ Dashboard do organizador para acompanhar quem já visualizou
- ✅ Interface responsiva para mobile
- ✅ Totalmente em Português Brasileiro

## 🏗️ Arquitetura

### Backend (Rust)
- **Framework**: Axum (rápido e moderno)
- **Banco de Dados**: SQLite (sem configuração necessária)
- **Email**: Lettre (suporte SMTP)
- **IDs**: ULIDs (ordenáveis e únicos)

### Frontend (TypeScript)
- **Framework**: SvelteKit (simples e eficiente)
- **Styling**: TailwindCSS
- **Build**: Vite
- **Output**: Static site (SPA)

## 📋 Pré-requisitos

### Para desenvolvimento local:
- Rust 1.85+ (`cargo --version`)
- Node.js 20+ (`node --version`)
- Conta de email SMTP (Gmail, etc.)

### Para produção com Docker:
- Docker e Docker Compose

## 🚀 Instalação e Execução

### Opção 1: Docker (Recomendado)

1. **Clone o repositório**
```bash
git clone <seu-repo>
cd amigo-oculto
```

2. **Configure as variáveis de ambiente**
```bash
cp backend/.env.example backend/.env
```

Edite `backend/.env` com suas configurações:
```env
DATABASE_URL=sqlite:///app/data/amigo_oculto.db
PORT=3000
BASE_URL=http://localhost:3000

# Gmail example (use App Password, não a senha normal)
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=seu-email@gmail.com
SMTP_PASSWORD=sua-senha-de-app
SMTP_FROM=noreply@amigooculto.app
```

3. **Build do frontend**
```bash
cd frontend
npm install
npm run build
cd ..
```

4. **Inicie os serviços**
```bash
docker-compose up -d
```

5. **Acesse a aplicação**
```
http://localhost:3000
```

### Opção 2: Desenvolvimento Local

1. **Configure o backend**
```bash
cd backend
cp .env.example .env
# Edite o .env com suas configurações
```

2. **Inicie o backend**
```bash
cargo run
```

3. **Em outro terminal, configure o frontend**
```bash
cd frontend
npm install
```

4. **Inicie o frontend em modo dev**
```bash
npm run dev
```

5. **Acesse**
- Frontend: http://localhost:5173
- API: http://localhost:3000/api

## 📧 Configurando Email (Gmail)

### 1. Criar App Password no Gmail

1. Acesse [myaccount.google.com](https://myaccount.google.com)
2. Vá em "Segurança"
3. Ative "Verificação em duas etapas" (se não estiver ativo)
4. Procure por "Senhas de app"
5. Crie uma senha de app para "Mail"
6. Use essa senha no campo `SMTP_PASSWORD` do `.env`

### 2. Configurações no .env

```env
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=seu-email@gmail.com
SMTP_PASSWORD=senha-de-app-gerada
SMTP_FROM=noreply@amigooculto.app  # Pode usar qualquer email aqui
```

### 3. Outros provedores de email

Para outros provedores (Outlook, Yahoo, provedor próprio), consulte a documentação do provedor para obter as configurações SMTP.

## 🎮 Como Usar

### Para o Organizador:

1. **Acesse a página inicial** e clique em "Criar Novo Jogo"
2. **Preencha os dados**: Nome do jogo, data do evento, seu email
3. **Adicione os participantes** um por um (nome + email)
4. **Revise a lista** de participantes
5. **Clique em "Realizar Sorteio"** - Os emails serão enviados automaticamente!
6. **Guarde o link** que você receber para acompanhar o status

### Para os Participantes:

1. **Receba o email** com o título "🎁 Amigo Oculto: [Nome do Jogo]"
2. **Clique no botão** "Ver Meu Amigo Oculto"
3. **Descubra quem você tirou!**
4. **Guarde o email** para consultar novamente se necessário

## 🛠️ Estrutura do Projeto

```
amigo-oculto/
├── backend/                 # Rust backend
│   ├── src/
│   │   ├── main.rs         # Entrada do servidor
│   │   ├── db.rs           # Operações do banco de dados
│   │   ├── email.rs        # Serviço de email
│   │   ├── matching.rs     # Lógica de sorteio
│   │   ├── models.rs       # Modelos de dados
│   │   └── routes.rs       # Endpoints da API
│   ├── Cargo.toml          # Dependências Rust
│   ├── Dockerfile
│   └── .env.example
├── frontend/                # SvelteKit frontend
│   ├── src/
│   │   ├── routes/         # Páginas
│   │   ├── lib/            # Bibliotecas compartilhadas
│   │   ├── app.html        # Template HTML base
│   │   └── app.css         # Estilos globais
│   ├── static/             # Arquivos estáticos
│   ├── package.json
│   └── svelte.config.js
├── data/                    # Banco de dados SQLite (criado automaticamente)
├── docker-compose.yml
└── README.md
```

## 🔧 API Endpoints

### Criar Jogo
```http
POST /api/games
Content-Type: application/json

{
  "name": "Natal da Família",
  "event_date": "25 de Dezembro",
  "organizer_email": "organizador@email.com"
}
```

### Adicionar Participante
```http
POST /api/games/:game_id/participants
Content-Type: application/json

{
  "name": "João Silva",
  "email": "joao@email.com"
}
```

### Realizar Sorteio
```http
POST /api/games/:game_id/draw
```

### Ver Status (Organizador)
```http
GET /api/games/:game_id?admin_token=xxx
```

### Ver Seu Amigo Oculto
```http
GET /api/reveal/:view_token
```

## 🚢 Deploy em Produção

### Raspberry Pi

1. **Instale Docker**
```bash
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER
```

2. **Clone e configure**
```bash
git clone <seu-repo> amigo-oculto
cd amigo-oculto
cp backend/.env.example backend/.env
nano backend/.env  # Configure suas variáveis
```

3. **Build frontend**
```bash
cd frontend
npm install
npm run build
cd ..
```

4. **Configure o BASE_URL** no `.env` com seu IP/domínio:
```env
BASE_URL=http://192.168.1.100:3000
# ou
BASE_URL=https://amigo.seudominio.com
```

5. **Inicie**
```bash
docker-compose up -d
```

### Serviços Cloud (Fly.io)

Este projeto está configurado para deploy no [Fly.io](https://fly.io). Commits na branch `main` são automaticamente deployados para staging via GitHub Actions.

Veja [CONTRIBUTING.md](CONTRIBUTING.md) para detalhes sobre o pipeline de CI/CD.

## 🔒 Segurança

- ✅ Tokens únicos gerados com criptografia segura (impossível adivinhar)
- ✅ IDs ordenáveis e únicos baseados em ULID
- ✅ Sem autenticação necessária (acesso via token)
- ✅ Organizador não consegue ver os pares sorteados
- ✅ Cada participante só vê seu próprio par
- ✅ Banco de dados SQLite local (fácil de fazer backup)

## 🐛 Troubleshooting

### Emails não estão sendo enviados

1. Verifique se o `SMTP_PASSWORD` está correto (use App Password do Gmail)
2. Verifique se a porta `587` está aberta no firewall
3. Veja os logs: `docker-compose logs -f backend`
4. Teste com outro provedor SMTP

### Erro ao conectar no banco de dados

1. Verifique se a pasta `data/` existe
2. Verifique permissões: `chmod 755 data/`
3. Delete o DB e reinicie: `rm data/amigo_oculto.db && docker-compose restart`

### Frontend não carrega

1. Certifique-se que você executou `npm run build` no frontend
2. Verifique se a pasta `frontend/build` existe
3. Reinicie o backend: `docker-compose restart`

## 📝 Licença

Este projeto é open source e está disponível sob a licença MIT.

## 🤝 Contribuindo

Contribuições são bem-vindas! Sinta-se à vontade para abrir issues ou pull requests.

## 💡 Futuras Melhorias

- [ ] Lista de desejos por participante
- [ ] Limite de orçamento sugerido
- [ ] Exclusão de pares (casais não tirarem um ao outro)
- [ ] Integração com WhatsApp
- [ ] Múltiplos organizadores
- [ ] Jogos recorrentes (salvar lista de participantes)
- [ ] Temas personalizados
- [ ] Export/backup dos jogos

## 📞 Suporte

Para dúvidas ou problemas, abra uma issue no GitHub ou entre em contato.

---

Feito com ❤️ para facilitar o Amigo Oculto da sua família!