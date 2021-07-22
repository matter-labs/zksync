<template>
    <span v-if="layer === 1" class="layer_icon mr-1">L1</span>
    <span v-else-if="layer === 2" class="layer_icon mr-1">L2</span>
</template>

<script>
export default {
    props: ['layer']
};
</script>
