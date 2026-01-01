<script lang="ts">
	import '../app.css';
</script>

<slot />

<footer class="fixed bottom-0 left-0 right-0 py-3 text-center text-sm bg-indigo-900/80 backdrop-blur-sm">
	<div class="text-white/70 space-x-4">
		<a href="/privacidade" class="hover:text-white transition-colors">Privacidade</a>
		<span>·</span>
		<a href="/termos" class="hover:text-white transition-colors">Termos de Uso</a>
	</div>
</footer>