<template >
    <router-link 
        :to="to"
    >
        <span class="link-html-span mr-1" v-html="innerHTML" />
    </router-link> 
</template>

<script>

export default {
    props: [
        'to',
        'innerHTML'
    ]
}
</script>

<style>

</style>