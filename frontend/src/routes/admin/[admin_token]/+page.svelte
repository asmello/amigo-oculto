<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';

	let adminToken: string | undefined;
	let gameData: any = null;
	let loading = true;
	let error = '';

	onMount(() => {
		adminToken = $page.params.admin_token;
		if (!adminToken) {
			error = 'Token de administrador não fornecido';
			loading = false;
			return;
		}
		loadGameData();
	});

	async function loadGameData() {
		try {
			// We need to find the game ID first - we'll try to extract it from the game data
			// For now, we'll need to modify the backend or pass game_id in the URL
			// Let's redirect to use the jogo route instead, or we need to update backend
			error = 'Use o link do email para acessar a página de administração do jogo';
		} catch (e: any) {
			error = e.message || 'Erro ao carregar dados';
			console.error(e);
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Administração - Amigo Oculto</title>
</svelte:head>

<div class="min-h-screen bg-cream flex items-center justify-center py-12 px-4 sm:px-6 lg:px-8">
	<div class="max-w-md w-full">
		<div class="bg-white rounded-lg shadow-xl p-8">
			<div class="text-center mb-6">
				<div class="text-6xl mb-4">📧</div>
				<h2 class="text-2xl font-bold text-gray-900 mb-2">Link de Administração</h2>
			</div>
			<div class="bg-blue-50 border border-blue-200 text-blue-700 px-4 py-3 rounded-lg text-center">
				Por favor, use o link completo que você recebeu por email para acessar a página de administração do seu jogo.
			</div>
		</div>
	</div>
</div>