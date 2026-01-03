<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';

	let gameId: string | undefined;
	let adminToken = '';
	let gameData: any = null;
	let loading = true;
	let error = '';

	let participantName = '';
	let participantEmail = '';
	let addingParticipant = false;
	let drawingGame = false;
	let drawSuccess = false;
	let showDeleteConfirm = false;
	let deleteConfirmName = '';
	let deleting = false;
	let resendingAll = false;
	let resendingId = '';
	let resendMessage = '';
	let editingId = '';
	let editName = '';
	let editEmail = '';
	let updating = false;

	onMount(() => {
		gameId = $page.params.game_id;
		adminToken = $page.url.searchParams.get('admin_token') || '';

		if (!gameId) {
			error = 'ID do jogo não fornecido';
			loading = false;
			return;
		}

		if (!adminToken) {
			error = 'Token de administrador não fornecido';
			loading = false;
			return;
		}

		loadGameData();
	});

	async function loadGameData() {
		try {
			const response = await fetch(`/api/games/${gameId}?admin_token=${adminToken}`);
			
			if (!response.ok) {
				throw new Error('Erro ao carregar dados do jogo');
			}
			
			gameData = await response.json();
		} catch (e) {
			error = 'Erro ao carregar jogo. Verifique se o link está correto.';
			console.error(e);
		} finally {
			loading = false;
		}
	}

	async function addParticipant() {
		if (!participantName || !participantEmail) {
			return;
		}

		addingParticipant = true;
		error = '';

		try {
			const response = await fetch(`/api/games/${gameId}/participants?admin_token=${adminToken}`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({
					name: participantName,
					email: participantEmail
				})
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao adicionar participante');
			}

			participantName = '';
			participantEmail = '';
			await loadGameData();
		} catch (e: any) {
			error = e.message || 'Erro ao adicionar participante';
			console.error(e);
		} finally {
			addingParticipant = false;
		}
	}

	async function performDraw() {
		// Prevent double-click/double-submission
		if (drawingGame) {
			return;
		}

		if (!confirm('Tem certeza que deseja realizar o sorteio? Os emails serão enviados para todos os participantes.')) {
			return;
		}

		drawingGame = true;
		error = '';

		try {
			const response = await fetch(`/api/games/${gameId}/draw?admin_token=${adminToken}`, {
				method: 'POST'
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao realizar sorteio');
			}

			drawSuccess = true;
			await loadGameData();
		} catch (e: any) {
			error = e.message || 'Erro ao realizar sorteio';
			console.error(e);
			drawingGame = false; // Re-enable button only on error
		}
		// Note: Don't set drawingGame = false on success, keep button disabled
	}

	async function resendAll() {
		if (!confirm('Tem certeza que deseja reenviar os emails para todos os participantes?')) {
			return;
		}

		resendingAll = true;
		error = '';
		resendMessage = '';

		try {
			const response = await fetch(`/api/games/${gameId}/resend-all?admin_token=${adminToken}`, {
				method: 'POST'
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao reenviar emails');
			}

			const data = await response.json();
			resendMessage = data.message;
			setTimeout(() => resendMessage = '', 5000);
		} catch (e: any) {
			error = e.message || 'Erro ao reenviar emails';
			console.error(e);
		} finally {
			resendingAll = false;
		}
	}

	async function resendOne(participantId: string, participantName: string) {
		if (!confirm(`Reenviar email para ${participantName}?`)) {
			return;
		}

		resendingId = participantId;
		error = '';
		resendMessage = '';

		try {
			const response = await fetch(
				`/api/games/${gameId}/participants/${participantId}/resend?admin_token=${adminToken}`,
				{ method: 'POST' }
			);

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao reenviar email');
			}

			const data = await response.json();
			resendMessage = data.message;
			setTimeout(() => resendMessage = '', 5000);
		} catch (e: any) {
			error = e.message || 'Erro ao reenviar email';
			console.error(e);
		} finally {
			resendingId = '';
		}
	}

	function startEdit(participant: any) {
		editingId = participant.id;
		editName = participant.name;
		editEmail = participant.email;
	}

	function cancelEdit() {
		editingId = '';
		editName = '';
		editEmail = '';
	}

	async function saveEdit(participantId: string) {
		updating = true;
		error = '';

		try {
			const response = await fetch(
				`/api/games/${gameId}/participants/${participantId}?admin_token=${adminToken}`,
				{
					method: 'PATCH',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({
						name: editName || undefined,
						email: editEmail || undefined
					})
				}
			);

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao atualizar participante');
			}

			cancelEdit();
			await loadGameData();
		} catch (e: any) {
			error = e.message || 'Erro ao atualizar participante';
			console.error(e);
		} finally {
			updating = false;
		}
	}

	async function confirmDelete() {
		if (deleteConfirmName !== gameData?.game.name) {
			error = 'O nome do jogo não corresponde. Digite exatamente como mostrado.';
			return;
		}

		deleting = true;
		error = '';

		try {
			const response = await fetch(`/api/games/${gameId}?admin_token=${adminToken}`, {
				method: 'DELETE'
			});

			if (!response.ok) {
				const errorData = await response.json();
				throw new Error(errorData.error || 'Erro ao excluir jogo');
			}

			// Redirect to home after successful deletion
			window.location.href = '/';
		} catch (e: any) {
			error = e.message || 'Erro ao excluir jogo';
			console.error(e);
			deleting = false;
			showDeleteConfirm = false;
		}
	}
</script>

<svelte:head>
	<title>Gerenciar Jogo - Amigo Oculto</title>
</svelte:head>

<div class="min-h-screen bg-cream py-12 px-4 sm:px-6 lg:px-8">
	<div class="max-w-4xl mx-auto">
		<div class="text-center mb-8">
			<a href="/" class="inline-block hover:scale-105 transition-transform cursor-pointer">
				<h1 class="text-4xl font-bold text-charcoal mb-2">🎁 Amigo Oculto</h1>
			</a>
		</div>


		{#if loading}
			<div class="bg-white rounded-lg shadow-xl p-8 text-center">
				<div class="text-gray-600">Carregando...</div>
			</div>
		{:else if error && !gameData}
			<div class="bg-white rounded-lg shadow-xl p-8">
				<div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg">
					{error}
				</div>
			</div>
		{:else if gameData}
			<div class="bg-white rounded-lg shadow-xl p-8 mb-6">
				<div class="border-b border-gray-200 pb-4 mb-6">
					<h2 class="text-2xl font-bold text-gray-900">{gameData.game.name}</h2>
					<p class="text-gray-600 mt-1">📅 {gameData.game.event_date}</p>
					{#if gameData.game.drawn}
						<div class="mt-3 inline-block bg-green-100 text-green-800 px-3 py-1 rounded-full text-sm font-semibold">
							✅ Sorteio Realizado
						</div>
					{:else}
						<div class="mt-3 inline-block bg-yellow-100 text-yellow-800 px-3 py-1 rounded-full text-sm font-semibold">
							⏳ Aguardando Sorteio
						</div>
					{/if}
				</div>

				{#if !gameData.game.drawn}
					<div class="mb-8">
						<h3 class="text-lg font-semibold text-gray-900 mb-4">Adicionar Participante</h3>
						
						<form on:submit|preventDefault={addParticipant} class="space-y-4">
							<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
								<div>
									<label for="participantName" class="block text-sm font-medium text-gray-700 mb-2">
										Nome
									</label>
									<input
										id="participantName"
										type="text"
										bind:value={participantName}
										placeholder="Nome do participante"
										required
										class="w-full px-4 py-2 border border-sage-light rounded-lg focus:ring-2 focus:ring-charcoal focus:border-transparent"
									/>
								</div>
								<div>
									<label for="participantEmail" class="block text-sm font-medium text-gray-700 mb-2">
										Email
									</label>
									<input
										id="participantEmail"
										type="email"
										bind:value={participantEmail}
										placeholder="email@exemplo.com"
										required
										class="w-full px-4 py-2 border border-sage-light rounded-lg focus:ring-2 focus:ring-charcoal focus:border-transparent"
									/>
								</div>
							</div>

							<button
								type="submit"
								disabled={addingParticipant}
								class="bg-charcoal text-white py-2 px-6 rounded-lg font-semibold hover:bg-charcoal-700 focus:outline-none focus:ring-2 focus:ring-charcoal focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
							>
								{addingParticipant ? 'Adicionando...' : 'Adicionar Participante'}
							</button>
						</form>
					</div>
				{/if}

				{#if error && gameData}
					<div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-4">
						{error}
					</div>
				{/if}

				{#if drawSuccess}
					<div class="bg-green-50 border border-green-200 text-green-700 px-4 py-3 rounded-lg mb-4">
						🎉 Sorteio realizado com sucesso! Emails enviados para todos os participantes.
					</div>
				{/if}

				{#if resendMessage}
					<div class="bg-blue-50 border border-blue-200 text-blue-700 px-4 py-3 rounded-lg mb-4">
						✉️ {resendMessage}
					</div>
				{/if}

				<div>
					<h3 class="text-lg font-semibold text-gray-900 mb-4">
						Participantes ({gameData.participants.length})
					</h3>

					{#if gameData.participants.length === 0}
						<div class="text-center py-8 text-gray-500">
							Nenhum participante adicionado ainda.
							<br />
							Adicione pelo menos 2 participantes para realizar o sorteio.
						</div>
					{:else}
						<div class="space-y-2 mb-6">
							{#each gameData.participants as participant}
								{#if editingId === participant.id}
									<div class="bg-blue-50 border border-blue-300 px-4 py-3 rounded-lg">
										<div class="space-y-2">
											<input
												type="text"
												bind:value={editName}
												placeholder="Nome"
												class="w-full px-3 py-2 border border-gray-300 rounded focus:ring-2 focus:ring-blue-600 focus:border-transparent"
											/>
											<input
												type="email"
												bind:value={editEmail}
												placeholder="Email"
												class="w-full px-3 py-2 border border-gray-300 rounded focus:ring-2 focus:ring-blue-600 focus:border-transparent"
											/>
											<div class="flex gap-2">
												<button
													on:click={() => saveEdit(participant.id)}
													disabled={updating}
													class="flex-1 bg-green-600 text-white px-3 py-1 rounded hover:bg-green-700 disabled:opacity-50 text-sm"
												>
													{updating ? 'Salvando...' : '✓ Salvar'}
												</button>
												<button
													on:click={cancelEdit}
													disabled={updating}
													class="flex-1 bg-gray-500 text-white px-3 py-1 rounded hover:bg-gray-600 disabled:opacity-50 text-sm"
												>
													✕ Cancelar
												</button>
											</div>
										</div>
									</div>
								{:else}
									<div class="flex items-center justify-between bg-gray-50 px-4 py-3 rounded-lg">
										<div class="flex-1">
											<div class="font-medium text-gray-900">{participant.name}</div>
											<div class="text-sm text-gray-600">{participant.email}</div>
										</div>
										<div class="flex items-center gap-2">
											{#if gameData.game.drawn}
												{#if participant.has_viewed}
													<span class="text-green-600 text-sm font-medium">✓ Visualizado</span>
												{:else}
													<span class="text-gray-400 text-sm font-medium">Não visualizado</span>
													<button
														on:click={() => startEdit(participant)}
														class="text-sm bg-gray-600 text-white px-2 py-1 rounded hover:bg-gray-700"
													>
														✏️
													</button>
												{/if}
												<button
													on:click={() => resendOne(participant.id, participant.name)}
													disabled={resendingId === participant.id}
													class="text-sm bg-blue-600 text-white px-3 py-1 rounded hover:bg-blue-700 disabled:opacity-50"
												>
													{resendingId === participant.id ? '...' : '📧'}
												</button>
											{:else}
												<button
													on:click={() => startEdit(participant)}
													class="text-sm bg-gray-600 text-white px-2 py-1 rounded hover:bg-gray-700"
												>
													✏️ Editar
												</button>
											{/if}
										</div>
									</div>
								{/if}
							{/each}
						</div>

						{#if !gameData.game.drawn}
							{#if gameData.participants.length >= 2}
								<button
									on:click={performDraw}
									disabled={drawingGame}
									class="w-full bg-sage text-charcoal-800 py-4 px-6 rounded-lg font-bold text-lg hover:bg-sage-400 focus:outline-none focus:ring-2 focus:ring-sage focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
								>
									{drawingGame ? '🎲 Realizando Sorteio...' : '🎲 Realizar Sorteio e Enviar Emails'}
								</button>
								<p class="text-sm text-gray-500 text-center mt-2">
									Cada participante receberá um email com um link para ver quem tirou
								</p>
							{:else}
								<div class="bg-yellow-50 border border-yellow-200 text-yellow-800 px-4 py-3 rounded-lg text-center">
									Adicione pelo menos 2 participantes para realizar o sorteio
								</div>
							{/if}
						{:else}
							<button
								on:click={resendAll}
								disabled={resendingAll}
								class="w-full bg-blue-600 text-white py-3 px-6 rounded-lg font-semibold hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-600 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
							>
								{resendingAll ? '📧 Reenviando...' : '📧 Reenviar Emails para Todos'}
							</button>
							<p class="text-sm text-gray-500 text-center mt-2">
								Útil se algum participante não recebeu o email ou perdeu o link
							</p>
						{/if}
					{/if}
				</div>
			</div>

			{#if gameData.game.drawn}
				<div class="bg-white/80 backdrop-blur-sm rounded-lg p-6 text-charcoal mb-6 shadow-lg">
					<h3 class="font-semibold mb-2">📧 Importante</h3>
					<p class="text-sm text-charcoal-700">
						Guarde este link! Você pode voltar aqui a qualquer momento para verificar quem já visualizou seu amigo oculto.
					</p>
					<p class="text-sm text-charcoal-700 mt-2">
						Os participantes que não visualizaram ainda podem ter perdido o email ou não verificaram a caixa de spam.
					</p>
				</div>
			{/if}

			<!-- Danger Zone: Delete Game -->
			<div class="bg-white rounded-lg shadow-xl p-6 border-2 border-red-200">
				<h3 class="text-lg font-semibold text-red-700 mb-2">⚠️ Zona de Perigo</h3>
				<p class="text-sm text-gray-600 mb-4">
					Esta ação é permanente e não pode ser desfeita. Todos os participantes e dados do sorteio serão excluídos.
				</p>
				<button
					on:click={() => { showDeleteConfirm = true; deleteConfirmName = ''; error = ''; }}
					class="bg-red-600 text-white py-2 px-4 rounded-lg font-semibold hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-600 focus:ring-offset-2 transition-all"
				>
					🗑️ Excluir Jogo
				</button>
			</div>
		{/if}
	</div>
</div>

<!-- Delete Confirmation Modal -->
{#if showDeleteConfirm}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
		<div class="bg-white rounded-lg shadow-xl max-w-md w-full p-6">
			<h2 class="text-2xl font-bold text-gray-900 mb-4">Confirmar Exclusão</h2>
			
			<div class="bg-red-50 border border-red-200 rounded-lg p-4 mb-4">
				<p class="text-sm text-red-800">
					<strong>⚠️ Atenção:</strong> Esta ação não pode ser desfeita!
				</p>
			</div>
			
			<p class="text-gray-700 mb-4">
				Para confirmar, digite o nome do jogo exatamente como mostrado:
			</p>
			
			<div class="bg-gray-50 border border-gray-300 rounded-lg p-3 mb-4">
				<p class="font-mono font-semibold text-gray-900">{gameData?.game.name}</p>
			</div>
			
			<input
				type="text"
				bind:value={deleteConfirmName}
				placeholder="Digite o nome do jogo aqui"
				class="w-full px-4 py-2 border border-sage-light rounded-lg focus:ring-2 focus:ring-red-600 focus:border-transparent mb-4"
			/>
			
			<div class="flex gap-3">
				<button
					on:click={() => { showDeleteConfirm = false; deleteConfirmName = ''; }}
					disabled={deleting}
					class="flex-1 bg-gray-200 text-gray-800 py-2 px-4 rounded-lg font-semibold hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-gray-400 focus:ring-offset-2 disabled:opacity-50 transition-all"
				>
					Cancelar
				</button>
				<button
					on:click={confirmDelete}
					disabled={deleting || deleteConfirmName !== gameData?.game.name}
					class="flex-1 bg-red-600 text-white py-2 px-4 rounded-lg font-semibold hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-600 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
				>
					{deleting ? 'Excluindo...' : 'Excluir Permanentemente'}
				</button>
			</div>
		</div>
	</div>
{/if}