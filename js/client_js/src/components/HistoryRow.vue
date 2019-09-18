<template>
    <b-card no-body class="mb-1">
        <b-card-header header-tag="header" class="p-1" role="tab">
            <b-row>
                <b-col>
                    <b-button 
                        :class="bButtonClasses"
                        @click="bButtonClasses.rotated = !bButtonClasses.rotated"
                        v-b-toggle='`${tx.elem_id}_body`' 
                        variant="outline-light">
                        <img src="../assets/expand-button.png" width="12em" height="12em" />
                    </b-button>
                </b-col>
                <b-col>
                    <b-input 
                        :id="`${tx.elem_id}_input`" 
                        @click="copyTestingCode(`${tx.elem_id}_input`)"
                        :value="tx.hash"
                        style="outline: none; box-shadow: none"
                        class="noselect"
                        readonly
                        ></b-input>
                </b-col>
            </b-row>
        </b-card-header>
        <b-collapse :id="`${tx.elem_id}_body`">
            <b-card-body>
                <b-card-text> type: {{ tx.type }} </b-card-text>
                <b-card-text> success: {{ tx.success }} </b-card-text>
                <b-card-text> fail_reason: {{ tx.fail_reason }} </b-card-text>
                <b-card-text> to: {{ tx.to }} </b-card-text>
                <b-card-text> amount: {{ tx.amount }} </b-card-text>
                <b-card-text> is_committed: {{ tx.is_committed }} </b-card-text>
                <b-card-text> is_verified: {{ tx.is_verified }} </b-card-text>
            </b-card-body>
        </b-collapse>
    </b-card>
</template>

<script>
export default {
    name: 'HistoryRow',
    props: ['tx'],
    data: () => ({
        bButtonClasses: {
            expandButton: true,
            rotated: true,
        },
    }),
    methods: {
        copyTestingCode (id) {
            let testingCodeToCopy = document.getElementById(id);
            // let string = `copied`;
            // let testingCodeToCopy = document.createElement('input');
            testingCodeToCopy.setAttribute('type', 'text');
            testingCodeToCopy.select();

            try {
                var successful = document.execCommand('copy');
                var msg = successful ? 'successful' : 'unsuccessful';
                console.log('Testing code was copied ' + msg);
            } catch (err) {
                alert('Oops, unable to copy');
            }

            /* unselect the range */
            // testingCodeToCopy.setAttribute('type', 'hidden');
            window.getSelection().removeAllRanges();
        },
    }
}
</script>

<style scoped>
.noselect {
  -webkit-touch-callout: none; /* iOS Safari */
    -webkit-user-select: none; /* Safari */
     -khtml-user-select: none; /* Konqueror HTML */
       -moz-user-select: none; /* Firefox */
        -ms-user-select: none; /* Internet Explorer/Edge */
            user-select: none; /* Non-prefixed version, currently
                                  supported by Chrome and Opera */
}

.rotated {
    transform: rotate(-90deg);
}

.expandButton {
    transition: all .25s ease;
}
</style>
