<script lang="ts">
	import { goto } from '$app/navigation';
	import { browser } from '$app/environment';

	let password = '';
	let loading = false;
	let error = '';

	// Check if already logged in
	if (browser) {
		const token = localStorage.getItem('site_admin_token');
		if (token) {
			goto('/site-admin');
		}
	}

	async function handleLogin() {
		if (!password) {
			error = 'Por favor, digite a senha';
			return;
		}

		loading = true;
		error = '';

		try {
			const response = await fetch('/api/site-admin/login', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({ password })
			});

			if (!response.ok) {
				const data = await response.json();
				throw new Error(data.error || 'Erro ao fazer login');
			}

			const data = await response.json();

			// Store token in localStorage
			localStorage.setItem('site_admin_token', data.session_token);
			localStorage.setItem('site_admin_expires', data.expires_at);

			// Redirect to admin panel
			goto('/site-admin');
		} catch (e: any) {
			error = e.message || 'Erro ao fazer login. Tente novamente.';
			password = '';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Admin - Login</title>
</svelte:head>

<div class="min-h-screen bg-gradient-to-br from-gray-800 via-gray-900 to-black py-12 px-4 sm:px-6 lg:px-8">
	<div class="max-w-md mx-auto">
		<div class="text-center mb-8">
			<h1 class="text-5xl font-bold text-white mb-2">🔐</h1>
			<h1 class="text-3xl font-bold text-white mb-2">Administração do Site</h1>
			<p class="text-gray-400">Acesso restrito</p>
		</div>

		<div class="bg-gray-800 rounded-lg shadow-xl p-8 border border-gray-700">
			<form on:submit|preventDefault={handleLogin} class="space-y-6">
				<div>
					<label for="password" class="block text-sm font-medium text-gray-300 mb-2">
						Senha de Administrador
					</label>
					<input
						id="password"
						type="password"
						bind:value={password}
						placeholder="Digite a senha"
						required
						autofocus
						class="w-full px-4 py-3 bg-gray-700 border border-gray-600 text-white rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent placeholder-gray-400"
					/>
				</div>

				{#if error}
					<div class="bg-red-900/30 border border-red-700 text-red-300 px-4 py-3 rounded-lg">
						{error}
					</div>
				{/if}

				<button
					type="submit"
					disabled={loading}
					class="w-full bg-gradient-to-r from-blue-600 to-blue-700 text-white py-3 px-4 rounded-lg font-semibold hover:from-blue-700 hover:to-blue-800 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-800 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
				>
					{loading ? 'Entrando...' : 'Entrar'}
				</button>
			</form>
		</div>

		<div class="mt-6 text-center">
			<a href="/" class="text-gray-400 hover:text-white text-sm">
				← Voltar para o site
			</a>
		</div>
	</div>
</div>
