<script lang="ts">
	import { goto } from '$app/navigation';
	import { browser } from '$app/environment';
	import { onMount } from 'svelte';

	type Game = {
		id: string;
		name: string;
		event_date: string;
		organizer_email: string;
		created_at: string;
		drawn: boolean;
		participant_count: number;
	};

	let token = '';
	let games: Game[] = [];
	let loading = true;
	let error = '';
	let searchQuery = '';
	let total = 0;
	let limit = 20;
	let offset = 0;
	let selectedGame: Game | null = null;
	let showDeleteConfirm = false;
	let deleting = false;

	// Check authentication
	onMount(() => {
		if (browser) {
			const storedToken = localStorage.getItem('site_admin_token');
			const expiresAt = localStorage.getItem('site_admin_expires');

			if (!storedToken || !expiresAt) {
				goto('/site-admin/login');
				return;
			}

			// Check if token is expired
			if (new Date(expiresAt) < new Date()) {
				localStorage.removeItem('site_admin_token');
				localStorage.removeItem('site_admin_expires');
				goto('/site-admin/login');
				return;
			}

			token = storedToken;
			loadGames();
		}
	});

	async function loadGames() {
		loading = true;
		error = '';

		try {
			const params = new URLSearchParams({
				limit: limit.toString(),
				offset: offset.toString()
			});

			if (searchQuery.trim()) {
				params.set('search', searchQuery.trim());
			}

			const response = await fetch(`/api/site-admin/games?${params}`, {
				headers: {
					'Authorization': `Bearer ${token}`
				}
			});

			if (response.status === 401) {
				// Token expired or invalid
				localStorage.removeItem('site_admin_token');
				localStorage.removeItem('site_admin_expires');
				goto('/site-admin/login');
				return;
			}

			if (!response.ok) {
				const data = await response.json();
				throw new Error(data.error || 'Erro ao carregar jogos');
			}

			const data = await response.json();
			games = data.games;
			total = data.total;
		} catch (e: any) {
			error = e.message || 'Erro ao carregar jogos';
		} finally {
			loading = false;
		}
	}

	function handleSearch() {
		offset = 0;
		loadGames();
	}

	function handleKeyPress(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			handleSearch();
		}
	}

	function nextPage() {
		offset += limit;
		loadGames();
	}

	function prevPage() {
		offset = Math.max(0, offset - limit);
		loadGames();
	}

	function formatDate(isoDate: string): string {
		const date = new Date(isoDate);
		return date.toLocaleDateString('pt-BR', {
			day: '2-digit',
			month: 'short',
			year: 'numeric'
		});
	}

	function formatDateTime(isoDateTime: string): string {
		const date = new Date(isoDateTime);
		return date.toLocaleString('pt-BR', {
			day: '2-digit',
			month: 'short',
			year: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function confirmDelete(game: Game) {
		selectedGame = game;
		showDeleteConfirm = true;
	}

	function cancelDelete() {
		selectedGame = null;
		showDeleteConfirm = false;
	}

	async function deleteGame() {
		if (!selectedGame) return;

		deleting = true;
		error = '';

		try {
			const response = await fetch(`/api/site-admin/games/${selectedGame.id}`, {
				method: 'DELETE',
				headers: {
					'Authorization': `Bearer ${token}`
				}
			});

			if (response.status === 401) {
				localStorage.removeItem('site_admin_token');
				localStorage.removeItem('site_admin_expires');
				goto('/site-admin/login');
				return;
			}

			if (!response.ok) {
				const data = await response.json();
				throw new Error(data.error || 'Erro ao excluir jogo');
			}

			// Reload games
			showDeleteConfirm = false;
			selectedGame = null;
			await loadGames();
		} catch (e: any) {
			error = e.message || 'Erro ao excluir jogo';
		} finally {
			deleting = false;
		}
	}

	function logout() {
		localStorage.removeItem('site_admin_token');
		localStorage.removeItem('site_admin_expires');
		goto('/site-admin/login');
	}

	$: currentPage = Math.floor(offset / limit) + 1;
	$: totalPages = Math.ceil(total / limit);
</script>

<svelte:head>
	<title>Painel de Administração</title>
</svelte:head>

<div class="min-h-screen bg-cream">
	<!-- Header -->
	<div class="bg-white border-b border-sage-light sticky top-0 z-10 shadow-sm">
		<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
			<div class="flex justify-between items-center">
				<div>
					<h1 class="text-2xl font-bold text-charcoal">Painel de Administração</h1>
					<p class="text-charcoal-600 text-sm">Gerenciamento de jogos</p>
				</div>
				<button
					on:click={logout}
					class="px-4 py-2 bg-charcoal-100 hover:bg-charcoal-200 text-charcoal rounded-lg transition-colors"
				>
					Sair
				</button>
			</div>
		</div>
	</div>

	<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
		<!-- Search -->
		<div class="mb-6">
			<div class="flex gap-2">
				<input
					type="text"
					bind:value={searchQuery}
					on:keypress={handleKeyPress}
					placeholder="Buscar por nome, email ou ID..."
					class="flex-1 px-4 py-2 bg-white border border-sage-light text-charcoal rounded-lg focus:ring-2 focus:ring-charcoal focus:border-transparent placeholder-charcoal-400"
				/>
				<button
					on:click={handleSearch}
					class="px-6 py-2 bg-charcoal hover:bg-charcoal-700 text-white rounded-lg font-medium transition-colors"
				>
					Buscar
				</button>
				{#if searchQuery}
					<button
						on:click={() => { searchQuery = ''; handleSearch(); }}
						class="px-6 py-2 bg-charcoal-200 hover:bg-charcoal-300 text-charcoal rounded-lg transition-colors"
					>
						Limpar
					</button>
				{/if}
			</div>
		</div>

		{#if error}
			<div class="mb-6 bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg">
				{error}
			</div>
		{/if}

		<!-- Stats -->
		<div class="mb-6 bg-white border border-sage-light rounded-lg p-4 shadow-sm">
			<p class="text-charcoal-600">
				Total de jogos: <span class="font-bold text-charcoal">{total}</span>
				{#if searchQuery}
					<span class="text-charcoal-400 text-sm ml-2">(filtrado)</span>
				{/if}
			</p>
		</div>

		<!-- Games List -->
		{#if loading}
			<div class="text-center py-12">
				<div class="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-charcoal"></div>
				<p class="text-charcoal-600 mt-4">Carregando...</p>
			</div>
		{:else if games.length === 0}
			<div class="text-center py-12 bg-white border border-sage-light rounded-lg shadow-sm">
				<p class="text-charcoal-600">
					{searchQuery ? 'Nenhum jogo encontrado com esse critério' : 'Nenhum jogo cadastrado'}
				</p>
			</div>
		{:else}
			<div class="space-y-4">
				{#each games as game}
					<div class="bg-white border border-sage-light rounded-lg p-6 hover:border-sage transition-colors shadow-sm">
						<div class="flex justify-between items-start">
							<div class="flex-1">
								<div class="flex items-center gap-3 mb-2">
									<h3 class="text-xl font-bold text-charcoal">{game.name}</h3>
									{#if game.drawn}
										<span class="px-2 py-1 bg-sage-100 border border-sage text-sage-700 text-xs rounded-full">
											Sorteado
										</span>
									{:else}
										<span class="px-2 py-1 bg-yellow-50 border border-yellow-300 text-yellow-700 text-xs rounded-full">
											Pendente
										</span>
									{/if}
								</div>

								<div class="grid grid-cols-1 md:grid-cols-2 gap-2 text-sm text-charcoal-500">
									<div>
										<span class="text-charcoal-400">ID:</span>
										<span class="text-charcoal-600 font-mono">{game.id}</span>
									</div>
									<div>
										<span class="text-charcoal-400">Organizador:</span>
										<span class="text-charcoal-600">{game.organizer_email}</span>
									</div>
									<div>
										<span class="text-charcoal-400">Data do evento:</span>
										<span class="text-charcoal-600">{formatDate(game.event_date)}</span>
									</div>
									<div>
										<span class="text-charcoal-400">Criado em:</span>
										<span class="text-charcoal-600">{formatDateTime(game.created_at)}</span>
									</div>
									<div>
										<span class="text-charcoal-400">Participantes:</span>
										<span class="text-charcoal-600">{game.participant_count}</span>
									</div>
								</div>
							</div>

							<div class="ml-4">
								<button
									on:click={() => confirmDelete(game)}
									class="px-4 py-2 bg-red-50 hover:bg-red-100 border border-red-200 text-red-700 rounded-lg text-sm font-medium transition-colors"
								>
									🗑️ Excluir
								</button>
							</div>
						</div>
					</div>
				{/each}
			</div>

			<!-- Pagination -->
			{#if totalPages > 1}
				<div class="mt-6 flex justify-center items-center gap-4">
					<button
						on:click={prevPage}
						disabled={offset === 0}
						class="px-4 py-2 bg-white hover:bg-cream-100 border border-sage-light text-charcoal rounded-lg disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
					>
						← Anterior
					</button>

					<span class="text-charcoal-600">
						Página {currentPage} de {totalPages}
					</span>

					<button
						on:click={nextPage}
						disabled={offset + limit >= total}
						class="px-4 py-2 bg-white hover:bg-cream-100 border border-sage-light text-charcoal rounded-lg disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
					>
						Próxima →
					</button>
				</div>
			{/if}
		{/if}
	</div>
</div>

<!-- Delete Confirmation Modal -->
{#if showDeleteConfirm && selectedGame}
	<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
		<div class="bg-white border border-sage-light rounded-lg p-6 max-w-md w-full shadow-xl">
			<h2 class="text-xl font-bold text-charcoal mb-4">Confirmar Exclusão</h2>

			<p class="text-charcoal-600 mb-4">
				Tem certeza que deseja excluir o jogo <span class="font-bold text-charcoal">"{selectedGame.name}"</span>?
			</p>

			<div class="bg-yellow-50 border border-yellow-200 text-yellow-800 px-4 py-3 rounded-lg mb-6 text-sm">
				⚠️ Esta ação é permanente e não pode ser desfeita. Todos os participantes e dados relacionados serão excluídos.
			</div>

			<div class="flex gap-3">
				<button
					on:click={cancelDelete}
					disabled={deleting}
					class="flex-1 px-4 py-2 bg-charcoal-100 hover:bg-charcoal-200 text-charcoal rounded-lg transition-colors disabled:opacity-50"
				>
					Cancelar
				</button>
				<button
					on:click={deleteGame}
					disabled={deleting}
					class="flex-1 px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50"
				>
					{deleting ? 'Excluindo...' : 'Excluir'}
				</button>
			</div>
		</div>
	</div>
{/if}
