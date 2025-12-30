<script lang="ts">
	import { goto } from '$app/navigation';
	
	let name = '';
	let eventDate = '';
	let organizerEmail = '';
	let loading = false;
	let error = '';
	let success = '';
	
	// Two-step flow
	let step: 'request' | 'verify' = 'request';
	let verificationId = '';
	let code = ['', '', '', '', '', ''];
	let codeInputs: HTMLInputElement[] = [];
	let timeRemaining = 15 * 60; // 15 minutes in seconds
	let timerInterval: number | null = null;
	let resendCooldown = 0;
	let resendInterval: number | null = null;

	// Set minimum date to today
	const today = new Date().toISOString().split('T')[0];

	// Format date for display in Brazilian format
	function formatBrazilianDate(isoDate: string): string {
		if (!isoDate) return '';
		const date = new Date(isoDate + 'T00:00:00');
		return date.toLocaleDateString('pt-BR', { 
			day: '2-digit', 
			month: 'long', 
			year: 'numeric',
			timeZone: 'UTC'
		});
	}

	function formatTime(seconds: number): string {
		const mins = Math.floor(seconds / 60);
		const secs = seconds % 60;
		return `${mins}:${secs.toString().padStart(2, '0')}`;
	}

	function startTimer() {
		if (timerInterval) clearInterval(timerInterval);
		timeRemaining = 15 * 60;
		timerInterval = setInterval(() => {
			timeRemaining--;
			if (timeRemaining <= 0) {
				if (timerInterval) clearInterval(timerInterval);
				error = 'Código expirado. Solicite um novo código.';
			}
		}, 1000) as any;
	}

	function startResendCooldown() {
		if (resendInterval) clearInterval(resendInterval);
		resendCooldown = 60;
		resendInterval = setInterval(() => {
			resendCooldown--;
			if (resendCooldown <= 0 && resendInterval) {
				clearInterval(resendInterval);
			}
		}, 1000) as any;
	}

	async function requestVerification() {
		if (!name || !eventDate || !organizerEmail) {
			error = 'Por favor, preencha todos os campos';
			return;
		}

		loading = true;
		error = '';
		success = '';

		try {
			const response = await fetch('/api/verifications/request', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({
					name,
					event_date: formatBrazilianDate(eventDate),
					organizer_email: organizerEmail
				})
			});

			if (!response.ok) {
				const data = await response.json();
				throw new Error(data.error || 'Erro ao enviar código');
			}

			const data = await response.json();
			verificationId = data.verification_id;
			step = 'verify';
			success = `Código enviado para ${organizerEmail}!`;
			startTimer();
			startResendCooldown();
			
			// Focus first input
			setTimeout(() => {
				if (codeInputs[0]) codeInputs[0].focus();
			}, 100);
		} catch (e: any) {
			error = e.message || 'Erro ao solicitar verificação. Tente novamente.';
			console.error(e);
		} finally {
			loading = false;
		}
	}

	function handleCodeInput(index: number, event: Event) {
		const input = event.target as HTMLInputElement;
		const value = input.value;

		// Only allow digits
		if (value && !/^\d$/.test(value)) {
			code[index] = '';
			return;
		}

		code[index] = value;

		// Auto-advance to next input
		if (value && index < 5) {
			codeInputs[index + 1]?.focus();
		}
	}

	function handleKeyDown(index: number, event: KeyboardEvent) {
		// Handle backspace
		if (event.key === 'Backspace' && !code[index] && index > 0) {
			codeInputs[index - 1]?.focus();
		}
	}

	function handlePaste(event: ClipboardEvent) {
		event.preventDefault();
		const pastedData = event.clipboardData?.getData('text');
		if (!pastedData) return;

		const digits = pastedData.replace(/\D/g, '').slice(0, 6);
		for (let i = 0; i < digits.length; i++) {
			code[i] = digits[i];
		}
		
		// Focus last filled input or last input
		const nextIndex = Math.min(digits.length, 5);
		codeInputs[nextIndex]?.focus();
	}

	async function verifyCode() {
		const fullCode = code.join('');
		if (fullCode.length !== 6) {
			error = 'Por favor, digite o código completo';
			return;
		}

		loading = true;
		error = '';

		try {
			const response = await fetch('/api/verifications/verify', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({
					verification_id: verificationId,
					code: fullCode
				})
			});

			const data = await response.json();

			if (!data.success) {
				error = data.error || 'Código incorreto';
				// Clear code inputs
				code = ['', '', '', '', '', ''];
				if (codeInputs[0]) codeInputs[0].focus();
				return;
			}

			// Success! Redirect to game page
			if (timerInterval) clearInterval(timerInterval);
			if (resendInterval) clearInterval(resendInterval);
			goto(`/jogo/${data.game_id}?admin_token=${data.admin_token}`);
		} catch (e) {
			error = 'Erro ao verificar código. Tente novamente.';
			console.error(e);
		} finally {
			loading = false;
		}
	}

	async function resendCode() {
		if (resendCooldown > 0) return;

		loading = true;
		error = '';
		success = '';

		try {
			const response = await fetch('/api/verifications/resend', {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({
					verification_id: verificationId
				})
			});

			const data = await response.json();

			if (!data.success) {
				error = data.error || 'Erro ao reenviar código';
				return;
			}

			success = 'Novo código enviado!';
			code = ['', '', '', '', '', ''];
			startTimer();
			startResendCooldown();
			if (codeInputs[0]) codeInputs[0].focus();
		} catch (e) {
			error = 'Erro ao reenviar código. Tente novamente.';
			console.error(e);
		} finally {
			loading = false;
		}
	}

	function goBack() {
		if (timerInterval) clearInterval(timerInterval);
		if (resendInterval) clearInterval(resendInterval);
		step = 'request';
		code = ['', '', '', '', '', ''];
		verificationId = '';
		error = '';
		success = '';
	}
</script>

<svelte:head>
	<title>Amigo Oculto - Criar Novo Jogo</title>
</svelte:head>

<div class="min-h-screen bg-gradient-to-br from-purple-600 via-purple-700 to-indigo-800 py-12 px-4 sm:px-6 lg:px-8">
	<div class="max-w-md mx-auto">
		<div class="text-center mb-8">
			<h1 class="text-5xl font-bold text-white mb-2">🎁</h1>
			<h1 class="text-4xl font-bold text-white mb-2">Amigo Oculto</h1>
			<p class="text-purple-200">Sistema de Sorteio Online</p>
		</div>

		<div class="bg-white rounded-lg shadow-xl p-8">
			<!-- Progress indicator -->
			<div class="mb-6 flex items-center justify-center space-x-4">
				<div class="flex items-center">
					<div class={`w-8 h-8 rounded-full flex items-center justify-center font-semibold ${step === 'request' ? 'bg-purple-600 text-white' : 'bg-green-500 text-white'}`}>
						{step === 'request' ? '1' : '✓'}
					</div>
					<span class="ml-2 text-sm font-medium text-gray-700">Informações</span>
				</div>
				<div class="w-12 h-1 bg-gray-300"></div>
				<div class="flex items-center">
					<div class={`w-8 h-8 rounded-full flex items-center justify-center font-semibold ${step === 'verify' ? 'bg-purple-600 text-white' : 'bg-gray-300 text-gray-600'}`}>
						2
					</div>
					<span class="ml-2 text-sm font-medium text-gray-700">Verificação</span>
				</div>
			</div>

			{#if step === 'request'}
				<h2 class="text-2xl font-bold text-gray-900 mb-6">Criar Novo Jogo</h2>

				<form on:submit|preventDefault={requestVerification} class="space-y-6">
					<div>
						<label for="name" class="block text-sm font-medium text-gray-700 mb-2">
							Nome do Jogo
						</label>
						<input
							id="name"
							type="text"
							bind:value={name}
							placeholder="Ex: Natal da Família Silva"
							required
							class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-purple-600 focus:border-transparent"
						/>
					</div>

					<div>
						<label for="eventDate" class="block text-sm font-medium text-gray-700 mb-2">
							Data do Evento
						</label>
						<input
							id="eventDate"
							type="date"
							bind:value={eventDate}
							min={today}
							required
							class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-purple-600 focus:border-transparent"
						/>
						{#if eventDate}
							<p class="text-sm text-gray-600 mt-1">
								📅 {formatBrazilianDate(eventDate)}
							</p>
						{/if}
					</div>

					<div>
						<label for="organizerEmail" class="block text-sm font-medium text-gray-700 mb-2">
							Seu Email (Organizador)
						</label>
						<input
							id="organizerEmail"
							type="email"
							bind:value={organizerEmail}
							placeholder="seu@email.com"
							required
							class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-purple-600 focus:border-transparent"
						/>
						<p class="text-xs text-gray-500 mt-1">
							Você receberá um código de verificação por email
						</p>
					</div>

					{#if error}
						<div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg">
							{error}
						</div>
					{/if}

					<button
						type="submit"
						disabled={loading}
						class="w-full bg-gradient-to-r from-purple-600 to-indigo-600 text-white py-3 px-4 rounded-lg font-semibold hover:from-purple-700 hover:to-indigo-700 focus:outline-none focus:ring-2 focus:ring-purple-600 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
					>
						{loading ? 'Enviando...' : 'Enviar Código de Verificação'}
					</button>
				</form>
			{:else}
				<div class="space-y-6">
					<div>
						<button
							on:click={goBack}
							class="text-purple-600 hover:text-purple-700 text-sm font-medium flex items-center mb-4"
						>
							← Voltar
						</button>
						<h2 class="text-2xl font-bold text-gray-900 mb-2">Verificar Email</h2>
						<p class="text-gray-600 text-sm">Digite o código de 6 dígitos enviado para {organizerEmail}</p>
					</div>

					{#if success}
						<div class="bg-green-50 border border-green-200 text-green-700 px-4 py-3 rounded-lg">
							{success}
						</div>
					{/if}

					<form on:submit|preventDefault={verifyCode}>
						<div class="flex justify-center space-x-2 mb-4" on:paste={handlePaste}>
							{#each code as digit, i}
								<input
									bind:this={codeInputs[i]}
									type="text"
									inputmode="numeric"
									maxlength="1"
									bind:value={code[i]}
									on:input={(e) => handleCodeInput(i, e)}
									on:keydown={(e) => handleKeyDown(i, e)}
									class="w-12 h-14 text-center text-2xl font-bold border-2 border-gray-300 rounded-lg focus:border-purple-600 focus:ring-2 focus:ring-purple-600 focus:outline-none"
								/>
							{/each}
						</div>

						<div class="text-center mb-4">
							<p class="text-sm text-gray-600">
								⏱️ Código expira em: <span class="font-semibold">{formatTime(timeRemaining)}</span>
							</p>
						</div>

						{#if error}
							<div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-4">
								{error}
							</div>
						{/if}

						<button
							type="submit"
							disabled={loading || code.join('').length !== 6}
							class="w-full bg-gradient-to-r from-purple-600 to-indigo-600 text-white py-3 px-4 rounded-lg font-semibold hover:from-purple-700 hover:to-indigo-700 focus:outline-none focus:ring-2 focus:ring-purple-600 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed transition-all mb-4"
						>
							{loading ? 'Verificando...' : 'Verificar e Criar Jogo'}
						</button>

						<div class="text-center">
							<button
								type="button"
								on:click={resendCode}
								disabled={resendCooldown > 0 || loading}
								class="text-purple-600 hover:text-purple-700 text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
							>
								{resendCooldown > 0 ? `Reenviar código (${resendCooldown}s)` : 'Não recebeu? Reenviar código'}
							</button>
						</div>
					</form>
				</div>
			{/if}
		</div>

		<div class="mt-8 text-center">
			<div class="bg-white/10 backdrop-blur-sm rounded-lg p-6 text-white">
				<h3 class="font-semibold mb-2">Como funciona?</h3>
				<ol class="text-sm text-left space-y-2 text-purple-100">
					<li>1. Verifique seu email para criar o jogo</li>
					<li>2. Adicione os participantes</li>
					<li>3. Realize o sorteio automático</li>
					<li>4. Cada um recebe um email com seu amigo oculto</li>
				</ol>
			</div>
		</div>
	</div>
</div>