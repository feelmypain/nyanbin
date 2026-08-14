<script lang="ts">
	import { t } from 'svelte-intl-precompile'
	import Create from '$lib/views/Create.svelte'
</script>
<svelte:head><title>{$t('create.page_title')} — Nyanbin</title><meta name="description" content={$t('create.meta_description')}/></svelte:head>
<Create />
