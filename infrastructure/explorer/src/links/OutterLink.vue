<template >
    <span>
        <a 
            :href="to" 
            target="_blank" 
            rel="noopener noreferrer"
        >
             <span class="link-html-span mr-1" v-html="innerHTML" />
        </a> 
        <i class="fas fa-external-link-alt mr-1"></i>
    </span>
</template>

<script>
export default {
    props: [
        'to',
        'innerHTML'
    ]
}
</script>

<style scoped>

</style>