<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';

	let viewToken = '';
	let revealData: any = null;
	let loading = true;
	let error = '';

	onMount(() => {
		viewToken = $page.params.view_token;
		loadRevealData();
	});

	async function loadRevealData() {
		try {
			const response = await fetch(`/api/reveal/${viewToken}`);
			
			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao carregar dados');
			}
			
			revealData = await response.json();
		} catch (e: any) {
			error = e.message || 'Link inválido ou expirado';
			console.error(e);
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Seu Amigo Oculto - Amigo Oculto</title>
</svelte:head>

<div class="min-h-screen bg-cream py-12 px-4 sm:px-6 lg:px-8">
	<div class="max-w-md mx-auto">
		<div class="text-center mb-8">
			<a href="/" class="inline-block hover:scale-105 transition-transform cursor-pointer">
				<h1 class="text-4xl font-bold text-charcoal mb-2">🎁 Amigo Oculto</h1>
			</a>
		</div>

		{#if loading}
			<div class="bg-white rounded-lg shadow-xl p-8 text-center">
				<div class="text-gray-600">Carregando...</div>
			</div>
		{:else if error}
			<div class="bg-white rounded-lg shadow-xl p-8">
				<div class="text-center mb-6">
					<div class="text-6xl mb-4">😕</div>
					<h2 class="text-2xl font-bold text-gray-900 mb-2">Oops!</h2>
				</div>
				<div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg text-center">
					{error}
				</div>
			</div>
		{:else if revealData}
			<div class="bg-white rounded-lg shadow-xl p-8">
				<div class="text-center mb-8">
					<div class="text-6xl mb-4">🎁</div>
					<h1 class="text-3xl font-bold text-gray-900 mb-2">Amigo Oculto</h1>
					<p class="text-gray-600">{revealData.game_name}</p>
				</div>

				<div class="bg-sage-50 rounded-lg p-6 mb-6 border-2 border-sage">
					<p class="text-center text-charcoal-700 mb-2">Olá, <span class="font-semibold">{revealData.your_name}</span>!</p>
					<p class="text-center text-charcoal-700 mb-4">Você tirou:</p>
					<div class="bg-white rounded-lg p-6 shadow-md">
						<p class="text-4xl font-bold text-center text-charcoal">
							{revealData.matched_name}
						</p>
					</div>
				</div>

				<div class="bg-yellow-50 border border-yellow-200 rounded-lg p-4 mb-6">
					<p class="text-sm text-yellow-800 text-center">
						<strong>📅 Data do evento:</strong> {revealData.event_date}
					</p>
				</div>

				<div class="bg-sage-50 border border-sage-200 rounded-lg p-4">
					<p class="text-sm text-charcoal-700 text-center">
						<strong>🤫 Lembre-se:</strong> Mantenha o segredo! A graça do amigo oculto é a surpresa no dia da troca.
					</p>
				</div>

				<div class="mt-6 text-center">
					<p class="text-xs text-gray-500">
						Guarde este link para consultar novamente se necessário
					</p>
				</div>
			</div>

			<div class="mt-8 text-center">
				<div class="bg-white/80 backdrop-blur-sm rounded-lg p-4 text-charcoal shadow-lg">
					<p class="text-sm">
						💡 <strong>Dica:</strong> Pense em um presente especial que a pessoa vai adorar!
					</p>
				</div>
			</div>
		{/if}
	</div>
</div>