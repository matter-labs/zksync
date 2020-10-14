<template>
    <i v-b-tooltip="text" class="far fa-question-circle displaynone"></i>
</template>

<script>
export default {
    name: 'Question',
    props: ['text'],
};
</script>
