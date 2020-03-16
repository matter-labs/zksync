<template>
    <b-button variant="light" 
        class="copyable tokenNameButton" 
        size="sm"
        v-b-tooltip.hover.left
        :title="hover_title"
        @click="clicked"
        @mouseenter="mouseEnter"
        :data-clipboard-text="data.item.address"
    > 
        <strong>{{ data.item.tokenName }} </strong>
    </b-button>
</template>

<script>
export default {
    name: 'TokenNameButton',
    props: ['data'],
    data: () => ({
        hover_title: '',
    }),
    methods: {
        clicked() {
            if (this.data.item.tokenName != 'ETH') {
                this.hover_title = 'copied';
            }
        },
        mouseEnter() {
            this.hover_title = this.data.item.tokenName != 'ETH'
                ? `Click to copy token address` 
                : `ETH token doesn't have address`;
        },
    },
}
</script>

<style scoped>
.tokenNameButton {
    display: inline-block;
    height: 2;
    width: 7em;
}
</style>
