<template>
    <span v-if="value">
        <layer-icon :layer="value.layer" />

        <inner-link v-if="value.isLocalLink"
            :to="value.to"
            :innerHTML="value.innerHTML"
        />
        <outter-link v-else-if="value.isOutterLink"
            :to="value.to"
            :innerHTML="value.innerHTML"
        />
        <span v-else class="link-html-span mr-1" v-html="value.innerHTML" />
        <span v-if="value.copyable" class="">
            <i v-if="value.tooltipRight===true"
                @click="clicked"
                class="far fa-copy cursorpointer" 
                v-b-tooltip.hover.right="hover_title"
                v-clipboard="value.innerHTML"
                @mouseenter="mouseEntered"
            ></i>
            <i v-else
                @click="clicked"
                class="far fa-copy cursorpointer" 
                v-b-tooltip="hover_title"
                v-clipboard="value.innerHTML"
                @mouseenter="mouseEntered"
            ></i>
        </span>
    </span>
</template>

<script>

import LayerIcon from './LayerIcon';
import InnerLink from './InnerLink';
import OutterLink from './OutterLink';

const components = {
    InnerLink,
    OutterLink,
    LayerIcon
}

export default {
    props: [
        'value',
    ],
    created() {
        console.log(this.value);
    },
    data: () => ({
        hover_title: ''
    }),
    methods: {
        clicked(event) {
            this.hover_title = "Copied";
            event.stopPropagation();
        },
        mouseEntered() {
            this.hover_title = "Click to copy";
        },
    },
    components
}
</script>

<style>
.link-html-span {
    font-size: 1.0em;
}
</style>