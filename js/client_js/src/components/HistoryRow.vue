<template>
    <b-card no-body class="mb-1">
        <b-card-header 
            header-tag="header" 
            class="p-1 noselect clickable" 
            role="tab"
            @click="cardHeaderClicked"  
            v-b-toggle='`${tx.data.elem_id}_body`'
        >
            <b-row>
                <b-col class="col-auto">
                    <b-button 
                        :class="bButtonClasses"
                        variant="outline-light">
                        <img src="../assets/expand-button.png" width="12em" height="12em" />
                    </b-button>
                </b-col>
                <b-col class="col-auto">
                    <span style="width: 5em" class="heightened"><b>{{ tx.data.type }}</b></span>
                </b-col>
                <b-col class="col-auto">
                    <div style="width: 7em">
                        <span style="font-weight: 300;" class="heightened">{{ tx.data.token }}</span> {{ tx.data.amount }}
                    </div>
                </b-col>
                <b-col class="col-auto">
                    <span v-html="tx.data.status" class="heightened"></span>
                </b-col>
                <b-col class="col-auto heightened">
                    <span v-if="tx.data.direction == 'incoming' " style="color: green; font-weight: bold" v-html="'<—'">
                    </span>
                    <span v-else style="color: red; font-weight: bold" v-html="'—>'">
                    </span>
                </b-col>
            </b-row>
        </b-card-header>
        <b-collapse :id="`${tx.data.elem_id}_body`">
            <b-card-body>
                <b-table 
                    stacked 
                    borderless 
                    small 
                    responsive 
                    :items="[tx.data]" 
                    :fields="tx.fields" 
                    class="ml-auto b-table-stacked-position-hack"
                >
                    <template v-slot:cell(amount)="data">
                        <span style="font-weight: 300">{{ tx.data.token }}</span> {{ tx.data.amount }}
                    </template>
                    <template v-slot:cell(to)="data">
                        <code class="clickable copyable" :data-clipboard-text="data.item.to">{{ data.item.to }}</code>
                    </template>
                    <template v-slot:cell(from)="data">
                        <code class="clickable copyable" :data-clipboard-text="data.item.from">{{ data.item.from }}</code>
                    </template>
                    <template v-slot:cell(row_status)="data">
                        <span v-html="data.item.row_status"></span>
                    </template>
                    <template v-slot:cell(hash)="data">
                        <code class="clickable copyable" :data-clipboard-text="data.item.hash">{{ data.item.hash }}</code>
                    </template>
                    <template v-slot:cell(pq_id)="data">
                        <code class="clickable copyable" :data-clipboard-text="data.item.pq_id">{{ data.item.pq_id }}</code>
                    </template>
                </b-table>
            </b-card-body>
        </b-collapse>
    </b-card>
</template>

<script>
import CopyableAddress from './CopyableAddress.vue'

const components = {
    CopyableAddress,
};

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
        cardHeaderClicked(event) {
            this.bButtonClasses.rotated = !this.bButtonClasses.rotated;
            event.preventDefault();
        }
    },
    components,
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

.clickable {
    cursor: pointer;
}

.rotated {
    transform: rotate(-90deg);
}

.expandButton {
    transition: all .25s ease;
}

.heightened {
    display: inline-block; 
    line-height: 2.3em;
}


.b-table-stacked-position-hack {
    position: relative; 
    left: -30%;
}

@media only screen and (max-width: 800px) {
    .b-table-stacked-position-hack {
        position: relative; 
        left: -20%;
    }
}

</style>
