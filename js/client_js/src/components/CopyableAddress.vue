<template>
    <b-form-input 
        type="text" 
        class="copyable mr-2"
        v-b-tooltip.hover
        :title="hover_title"
        :data-clipboard-text="address"
        :value="address"
        @click="clicked"
        @mouseenter="mouseEntered"
        readonly bg-variant="light"
    ></b-form-input>
</template>

<script>
export default {
    name: 'CopyableAddress',
    props: ['address'],
    data: () => ({
        hover_title: ''
    }),
    methods: {
        clicked() {
            this.hover_title = "Copied"
        },
        mouseEntered() {
            this.hover_title = "Click to copy"
        },
    },
}
</script>
