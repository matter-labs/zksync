<template>
    <router-link :to="to">
        <span class="link-html-span" v-html="innerHTML" />
    </router-link>
</template>

<script>
export default {
    props: ['to', 'innerHTML']
};
</script>

<style></style>
