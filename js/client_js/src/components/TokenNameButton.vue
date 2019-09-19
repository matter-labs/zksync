<template>
    <b-button variant="light" 
        class="copyable tokenNameButton" 
        size="sm"
        v-b-tooltip.hover 
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
            this.hover_title = 'copied';
        },
        mouseEnter() {
            this.hover_title = this.data.item.address 
                ? `Click to copy token address` : `ETH token doesn't have address`;
        },
    },
}
</script>
